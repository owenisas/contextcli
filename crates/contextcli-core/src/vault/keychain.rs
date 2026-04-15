//! macOS Keychain credential store.
//!
//! # The "always allow" problem — and the fix
//!
//! `SecKeychainAddGenericPassword` attaches a **per-application ACL** to every
//! item, keyed by the binary's code-signature hash.  macOS therefore shows
//! *"contextcli wants to use your keychain"* once per item — and **again after
//! every `cargo build`** because the binary hash changes.  With 7+ profiles
//! the user can see 10+ prompts on every rebuild.
//!
//! The fix: immediately after creating each item we call
//! `SecKeychainItemSetAccess` with a `SecAccess` whose ACL contains a single
//! **"any-application"** trusted entry (`SecTrustedApplicationCreateFromPath(NULL)`)
//! with **no confirmation flag**.  Any process can then read the item silently,
//! regardless of binary hash.
//!
//! # Migration
//!
//! Items stored before this change still carry the old per-app ACL.  On the
//! first successful `retrieve` we delete and re-store the item, which gives it
//! the permissive ACL.  The migration triggers the legacy prompt **exactly
//! once** per item; after that the prompt never appears again.

use crate::error::{Error, Result};
use crate::vault::CredentialStore;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFRelease, CFTypeRef, OSStatus};
use security_framework_sys::base::{SecKeychainItemRef, SecKeychainRef};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
};
use security_framework_sys::keychain::SecKeychainAddGenericPassword;
use std::ffi::c_void;
use std::ptr;

// OSStatus codes
const ERR_SEC_SUCCESS: OSStatus = 0;
const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
/// Returned by SecItemCopyMatching when the item exists but requires user
/// interaction (dialog) that we suppressed with kSecUseAuthenticationUIFail.
const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25315;

// ── FFI declarations not exported by security-framework-sys ──────────────

type SecAccessRef = *mut c_void;
type SecTrustedApplicationRef = *mut c_void;

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    /// Create a `SecAccess` with a given trusted-application list.
    /// An **empty** `CFArray` (not NULL) means "any application is trusted".
    fn SecAccessCreate(
        descriptor: core_foundation_sys::string::CFStringRef,
        trustedlist: core_foundation_sys::array::CFArrayRef,
        access: *mut SecAccessRef,
    ) -> OSStatus;

    /// Create a trusted-application token.  `path = NULL` → "any application".
    fn SecTrustedApplicationCreateFromPath(
        path: *const std::ffi::c_char,
        app: *mut SecTrustedApplicationRef,
    ) -> OSStatus;

    /// Attach an access-control object to a keychain item.
    fn SecKeychainItemSetAccess(itemRef: SecKeychainItemRef, access: SecAccessRef) -> OSStatus;

    /// Query the keychain without triggering any UI.
    fn SecItemCopyMatching(
        query: core_foundation_sys::dictionary::CFDictionaryRef,
        result: *mut core_foundation_sys::base::CFTypeRef,
    ) -> OSStatus;

    /// Key for controlling whether SecItemCopyMatching may show UI.
    /// Value: "fail" → return errSecInteractionNotAllowed instead of prompting.
    static kSecUseAuthenticationUI: core_foundation_sys::string::CFStringRef;
}

// ── KeychainStore ─────────────────────────────────────────────────────────

pub struct KeychainStore;

impl KeychainStore {
    pub fn new() -> Self {
        Self
    }
}

impl CredentialStore for KeychainStore {
    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()> {
        // Clean slate: remove any existing item first (handles old-ACL and new-ACL items).
        let _ = self.delete(service, account);

        unsafe {
            let mut item_ref: SecKeychainItemRef = ptr::null_mut();
            let status = SecKeychainAddGenericPassword(
                ptr::null_mut() as SecKeychainRef, // default keychain
                service.len() as u32,
                service.as_ptr() as *const _,
                account.len() as u32,
                account.as_ptr() as *const _,
                secret.len() as u32,
                secret.as_ptr() as *const _,
                &mut item_ref,
            );
            if status != ERR_SEC_SUCCESS {
                return Err(Error::Vault(format!(
                    "keychain store failed (OSStatus {status})"
                )));
            }

            // Attach a permissive ACL: any app can read without prompting.
            make_any_app_access(item_ref);

            // SecKeychainAddGenericPassword returns a +1-retained ref; release it.
            if !item_ref.is_null() {
                CFRelease(item_ref as CFTypeRef);
            }
        }
        Ok(())
    }

    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
        use security_framework::passwords::get_generic_password;

        let bytes = get_generic_password(service, account)
            .map_err(|e| Error::Vault(format!("keychain retrieve failed: {e}")))?;

        // Re-store to silently migrate old-ACL items to the permissive ACL.
        // After this first migration the re-store is a no-op from the user's
        // perspective (no prompts, fast operation).
        if self.store(service, account, &bytes).is_ok() {
            tracing::debug!("refreshed keychain ACL for {service}/{account}");
        }

        Ok(bytes)
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        use security_framework::passwords::delete_generic_password;
        match delete_generic_password(service, account) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND as i32 => Ok(()),
            Err(e) => Err(Error::Vault(format!("keychain delete failed: {e}"))),
        }
    }

    /// Silently checks whether the item requires a one-time user authorization.
    ///
    /// Uses `SecItemCopyMatching` with `kSecUseAuthenticationUIFail` so macOS
    /// returns `errSecInteractionNotAllowed` instead of showing a dialog.
    /// Returns `true`  → item exists but locked behind the old per-app ACL.
    /// Returns `false` → item is accessible, missing, or on an error.
    fn needs_auth(&self, service: &str, account: &str) -> bool {
        // "fail" is the documented string value of kSecUseAuthenticationUIFail.
        let ui_fail = CFString::new("fail");

        let query = CFDictionary::<CFType, CFType>::from_CFType_pairs(&[
            (
                unsafe { CFString::wrap_under_get_rule(kSecClass) }.as_CFType(),
                unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecAttrService) }.as_CFType(),
                CFString::new(service).as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) }.as_CFType(),
                CFString::new(account).as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecUseAuthenticationUI) }.as_CFType(),
                ui_fail.as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecReturnData) }.as_CFType(),
                CFBoolean::false_value().as_CFType(),
            ),
        ]);

        let mut result: CFTypeRef = ptr::null_mut();
        let status = unsafe {
            SecItemCopyMatching(
                query.as_concrete_TypeRef() as _,
                &mut result,
            )
        };
        if !result.is_null() {
            unsafe { CFRelease(result) };
        }

        status == ERR_SEC_INTERACTION_NOT_ALLOWED
    }
}

// ── ACL helper ────────────────────────────────────────────────────────────

/// Attach an access object to `item_ref` that allows **any application** to
/// read the item without user confirmation.
///
/// Steps:
/// 1. `SecTrustedApplicationCreateFromPath(NULL)` → "any app" sentinel
/// 2. `SecAccessCreate(label, [any_app_sentinel])` → access with one entry
/// 3. `SecKeychainItemSetAccess(item_ref, access)` → attach it
///
/// Errors are non-fatal (logged only).  If this step fails the item still
/// exists with the default single-app ACL — the legacy prompt behaviour.
unsafe fn make_any_app_access(item_ref: SecKeychainItemRef) {
    if item_ref.is_null() {
        return;
    }

    // Step 1: "any application" trusted-application reference
    let mut any_app: SecTrustedApplicationRef = ptr::null_mut();
    let s1 = unsafe { SecTrustedApplicationCreateFromPath(ptr::null(), &mut any_app) };
    if s1 != ERR_SEC_SUCCESS || any_app.is_null() {
        tracing::warn!("SecTrustedApplicationCreateFromPath failed: {s1}");
        return;
    }

    // Step 2: build CFArray([any_app]) and create the access
    let trusted: CFArray<CFType> = unsafe {
        CFArray::from_CFTypes(&[CFType::wrap_under_create_rule(any_app as CFTypeRef)])
    };
    let label = CFString::new("contextcli credential");
    let mut access: SecAccessRef = ptr::null_mut();
    let s2 = unsafe {
        SecAccessCreate(
            label.as_concrete_TypeRef(),
            trusted.as_concrete_TypeRef() as _,
            &mut access,
        )
    };
    if s2 != ERR_SEC_SUCCESS || access.is_null() {
        tracing::warn!("SecAccessCreate failed: {s2}");
        // `trusted` (CFArray) already holds the only retain on `any_app`; it will
        // release it when dropped here — no manual CFRelease needed.
        return;
    }

    // Step 3: attach the permissive access to the item
    let s3 = unsafe { SecKeychainItemSetAccess(item_ref, access) };
    if s3 != ERR_SEC_SUCCESS {
        tracing::warn!("SecKeychainItemSetAccess failed: {s3}");
    }

    unsafe {
        CFRelease(access as CFTypeRef);
        // `trusted` (CFArray) owns the `any_app` retain and will CFRelease it
        // when `trusted` is dropped at the end of this function — do NOT release
        // `any_app` manually or the retain count goes to zero while the array
        // still holds a reference (→ SIGSEGV).
    }
}

// ── tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SVC: &str = "contextcli-test-ephemeral";
    const ACCT: &str = "test/unit/token";

    #[test]
    fn test_store_retrieve_delete() {
        let store = KeychainStore::new();
        let secret = b"test-secret-value-12345";

        store.store(SVC, ACCT, secret).unwrap();
        let retrieved = store.retrieve(SVC, ACCT).unwrap();
        assert_eq!(retrieved, secret);

        let new_secret = b"updated-secret";
        store.store(SVC, ACCT, new_secret).unwrap();
        let retrieved = store.retrieve(SVC, ACCT).unwrap();
        assert_eq!(retrieved, new_secret);

        store.delete(SVC, ACCT).unwrap();
        assert!(store.retrieve(SVC, ACCT).is_err());
    }

    #[test]
    fn test_retrieve_nonexistent() {
        let store = KeychainStore::new();
        assert!(store.retrieve(SVC, "nonexistent/key/here").is_err());
    }

    #[test]
    fn test_exists() {
        let store = KeychainStore::new();
        let account = "test/exists/token";

        let _ = store.delete(SVC, account);
        assert!(!store.exists(SVC, account));
        store.store(SVC, account, b"val").unwrap();
        assert!(store.exists(SVC, account));
        store.delete(SVC, account).unwrap();
        assert!(!store.exists(SVC, account));
    }
}
