//! macOS Keychain credential store.
//!
//! # The "so many dialogs" problem — root cause and fix
//!
//! ## What was wrong
//!
//! The previous approach called `SecKeychainItemSetAccess` after storing each
//! item to attach a permissive ACL.  This turned ONE store operation into FOUR
//! dialogs:
//!
//!   1. "wants to use your confidential information"  (Always Allow ✓)
//!   2. "wants to change access permissions"          (**password required** ✗)
//!   3. "wants to access key 'contextcli'"            (Always Allow ✓)
//!   4. "wants to change the owner"                   (**password required** ✗)
//!
//! Dialogs 2 and 4 come from `SecKeychainItemSetAccess` and **can never be
//! dismissed with Always Allow** — they require the login keychain password
//! every single time.
//!
//! ## The fix
//!
//! Use `SecItemAdd` with `kSecAttrAccess` set to a `SecAccess` whose trusted
//! list is an **empty CFArray** (= "any application is trusted, no
//! confirmation needed").  This sets the permissive ACL at creation time so
//! `SecKeychainItemSetAccess` is never called.
//!
//! Result per profile:
//!   • New items:      0 dialogs ever
//!   • Existing items: 1 "Always Allow" dialog on first read (migration),
//!                     then 0 dialogs forever regardless of `cargo build`
//!
//! ## Migration
//!
//! On the first `retrieve` of an old-ACL item we try to silently delete it
//! and re-store it with the new permissive ACL.  If the delete fails (item
//! owned by a previous binary that no longer matches), we skip migration and
//! return the bytes unchanged — no password prompts are ever generated.

use crate::error::{Error, Result};
use crate::vault::CredentialStore;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFRelease, CFTypeRef, OSStatus};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
    kSecValueData,
};
use std::ffi::c_void;
use std::ptr;

// OSStatus codes
const ERR_SEC_SUCCESS: OSStatus = 0;
const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
/// Returned when the item exists but requires user interaction that we
/// suppressed with kSecUseAuthenticationUI = "fail".
const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25315;

// ── FFI declarations not in security-framework-sys ───────────────────────

type SecAccessRef = *mut c_void;

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    /// Create a `SecAccess` with a trusted-application list.
    /// **Empty `CFArray`** (not NULL) = any application trusted, no confirmation.
    fn SecAccessCreate(
        descriptor: core_foundation_sys::string::CFStringRef,
        trustedlist: core_foundation_sys::array::CFArrayRef,
        access: *mut SecAccessRef,
    ) -> OSStatus;

    /// Add a new keychain item.  Accepts `kSecAttrAccess` to set the ACL at
    /// creation time — this is the key to avoiding `SecKeychainItemSetAccess`.
    fn SecItemAdd(
        attributes: core_foundation_sys::dictionary::CFDictionaryRef,
        result: *mut CFTypeRef,
    ) -> OSStatus;

    /// Query the keychain.  With `kSecUseAuthenticationUI = "fail"` this
    /// returns `errSecInteractionNotAllowed` instead of showing a dialog when
    /// the item exists but the caller is not in its ACL.
    fn SecItemCopyMatching(
        query: core_foundation_sys::dictionary::CFDictionaryRef,
        result: *mut CFTypeRef,
    ) -> OSStatus;

    /// Keychain item attribute whose value is a `SecAccessRef` — sets ACL at
    /// creation time when passed to `SecItemAdd`.
    static kSecAttrAccess: core_foundation_sys::string::CFStringRef;

    /// Controls whether `SecItemCopyMatching` may show UI.
    /// String value "fail" → return `errSecInteractionNotAllowed` instead.
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
        // Remove any existing item first (old-ACL or new-ACL).
        let _ = self.delete(service, account);

        unsafe {
            // Build a permissive SecAccess: empty trusted list = any app, no prompt.
            let label = CFString::new("contextcli credential");
            let empty_trusted: CFArray<CFType> = CFArray::from_CFTypes(&[]);
            let mut access: SecAccessRef = ptr::null_mut();
            let sa = SecAccessCreate(
                label.as_concrete_TypeRef(),
                empty_trusted.as_concrete_TypeRef() as _,
                &mut access,
            );
            if sa != ERR_SEC_SUCCESS || access.is_null() {
                tracing::warn!("SecAccessCreate failed ({sa}); storing without permissive ACL");
                return store_legacy(service, account, secret);
            }

            // Wrap access into a CFType so the dictionary manages its lifetime.
            let cf_access = CFType::wrap_under_create_rule(access as CFTypeRef);
            let data = CFData::from_buffer(secret);

            let query = CFDictionary::<CFType, CFType>::from_CFType_pairs(&[
                (
                    CFString::wrap_under_get_rule(kSecClass).as_CFType(),
                    CFString::wrap_under_get_rule(kSecClassGenericPassword).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrService).as_CFType(),
                    CFString::new(service).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrAccount).as_CFType(),
                    CFString::new(account).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecValueData).as_CFType(),
                    data.as_CFType(),
                ),
                (
                    // Set permissive ACL at creation time — no SecKeychainItemSetAccess needed.
                    CFString::wrap_under_get_rule(kSecAttrAccess).as_CFType(),
                    cf_access.as_CFType(),
                ),
            ]);

            let status = SecItemAdd(query.as_concrete_TypeRef() as _, ptr::null_mut());
            if status != ERR_SEC_SUCCESS {
                return Err(Error::Vault(format!(
                    "keychain store failed (OSStatus {status})"
                )));
            }
            // cf_access, data, query all drop here — CFDictionary releases access.
        }
        Ok(())
    }

    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
        use security_framework::passwords::get_generic_password;

        let bytes = get_generic_password(service, account)
            .map_err(|e| Error::Vault(format!("keychain retrieve failed: {e}")))?;

        // Migration: try to re-store with permissive ACL.
        // We first delete the old item, then create a new one.  If delete fails
        // (e.g. the item is owned by an older binary we can no longer impersonate)
        // we skip silently — NO password prompts are generated either way.
        use security_framework::passwords::delete_generic_password;
        if delete_generic_password(service, account).is_ok() {
            if self.store(service, account, &bytes).is_ok() {
                tracing::debug!("migrated keychain ACL for {service}/{account}");
            }
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

    /// Silently checks whether the item requires one-time user authorization.
    ///
    /// Uses `SecItemCopyMatching` with `kSecUseAuthenticationUI = "fail"` so
    /// macOS returns `errSecInteractionNotAllowed` instead of showing a dialog.
    ///
    /// `true`  → item exists but is locked behind an old per-app ACL.
    /// `false` → item is accessible, missing, or on any error.
    fn needs_auth(&self, service: &str, account: &str) -> bool {
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
        let status =
            unsafe { SecItemCopyMatching(query.as_concrete_TypeRef() as _, &mut result) };
        if !result.is_null() {
            unsafe { CFRelease(result) };
        }

        status == ERR_SEC_INTERACTION_NOT_ALLOWED
    }
}

// ── Fallback ──────────────────────────────────────────────────────────────

/// Store using the legacy API without a permissive ACL.  Used only when
/// `SecAccessCreate` fails (should be extremely rare).  Items stored this way
/// will prompt once per new binary hash, but never generate password dialogs.
unsafe fn store_legacy(service: &str, account: &str, secret: &[u8]) -> Result<()> {
    use security_framework_sys::base::SecKeychainRef;
    use security_framework_sys::keychain::SecKeychainAddGenericPassword;

    let mut item_ref: security_framework_sys::base::SecKeychainItemRef = ptr::null_mut();
    let status = unsafe {
        SecKeychainAddGenericPassword(
            ptr::null_mut() as SecKeychainRef,
            service.len() as u32,
            service.as_ptr() as *const _,
            account.len() as u32,
            account.as_ptr() as *const _,
            secret.len() as u32,
            secret.as_ptr() as *const _,
            &mut item_ref,
        )
    };
    if !item_ref.is_null() {
        unsafe { CFRelease(item_ref as CFTypeRef) };
    }
    if status != ERR_SEC_SUCCESS {
        return Err(Error::Vault(format!(
            "keychain store (legacy) failed (OSStatus {status})"
        )));
    }
    Ok(())
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
