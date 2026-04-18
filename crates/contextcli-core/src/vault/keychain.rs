//! macOS Keychain credential store.
//!
//! # How silent access works
//!
//! Every keychain item has an ACL that lists which applications may read it.
//! By default `SecKeychainAddGenericPassword` ties the item to the creating
//! binary's code-signature hash — so every `cargo build` (new hash) triggers
//! a new prompt.
//!
//! The fix: use `SecItemAdd` with `kSecAttrAccess` set to a `SecAccess` whose
//! trusted list contains **`SecTrustedApplicationCreateFromPath(NULL)`** — the
//! "any application" sentinel.  This bakes the permissive ACL in at creation
//! time, so:
//!
//!   • New items   → 0 dialogs ever
//!   • Old items   → 1 "Always Allow" on first read (migration), then 0 forever
//!
//! Crucially, `SecKeychainItemSetAccess` is **never called**.  That function
//! requires the user's login-keychain password to change ownership and always
//! generates password-required dialogs that cannot be dismissed with
//! "Always Allow".

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
const ERR_SEC_DUPLICATE_ITEM: OSStatus = -25299;
const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25315;

// ── FFI ──────────────────────────────────────────────────────────────────

type SecAccessRef = *mut c_void;
type SecTrustedApplicationRef = *mut c_void;

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    /// `path = NULL` → "any application" sentinel.
    fn SecTrustedApplicationCreateFromPath(
        path: *const std::ffi::c_char,
        app: *mut SecTrustedApplicationRef,
    ) -> OSStatus;

    /// Create a `SecAccess`.  Trusted list containing the any-app sentinel
    /// produced by `SecTrustedApplicationCreateFromPath(NULL)` means any
    /// application may read without confirmation.
    fn SecAccessCreate(
        descriptor: core_foundation_sys::string::CFStringRef,
        trustedlist: core_foundation_sys::array::CFArrayRef,
        access: *mut SecAccessRef,
    ) -> OSStatus;

    /// Add a new keychain item.  Pass `kSecAttrAccess` to set ACL at creation.
    fn SecItemAdd(
        attributes: core_foundation_sys::dictionary::CFDictionaryRef,
        result: *mut CFTypeRef,
    ) -> OSStatus;

    /// Query keychain.  With `kSecUseAuthenticationUI = "fail"` returns
    /// `errSecInteractionNotAllowed` instead of showing a dialog.
    fn SecItemCopyMatching(
        query: core_foundation_sys::dictionary::CFDictionaryRef,
        result: *mut CFTypeRef,
    ) -> OSStatus;

    /// Delete keychain items matching the query.  Metadata-only op; never
    /// prompts and ignores ACLs (unlike the legacy `SecKeychainItemDelete`).
    fn SecItemDelete(
        query: core_foundation_sys::dictionary::CFDictionaryRef,
    ) -> OSStatus;

    /// Update attributes/value of matching items.
    fn SecItemUpdate(
        query: core_foundation_sys::dictionary::CFDictionaryRef,
        attributes_to_update: core_foundation_sys::dictionary::CFDictionaryRef,
    ) -> OSStatus;

    /// Attribute key: set a `SecAccessRef` on the item at creation time.
    static kSecAttrAccess: core_foundation_sys::string::CFStringRef;

    /// Key to suppress UI in `SecItemCopyMatching`.  Value: CFString "fail".
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
        // Remove any existing item via SecItemDelete (metadata-only; no ACL check,
        // no prompt).  The legacy delete_generic_password requires the caller to
        // be in the item's trusted list, so items created by a different binary
        // hash would otherwise block re-login with errSecDuplicateItem.
        unsafe { sec_item_delete(service, account) };

        unsafe {
            // Build permissive ACL: [any_app_sentinel] → any process reads silently.
            let access = make_any_app_access();
            let cf_access = match access {
                Some(a) => CFType::wrap_under_create_rule(a as CFTypeRef),
                None => {
                    // Rare fallback: store without permissive ACL.  Will prompt once
                    // per binary hash but will never ask for a password.
                    tracing::warn!("SecAccessCreate failed; storing without permissive ACL");
                    return store_legacy(service, account, secret);
                }
            };

            let data = CFData::from_buffer(secret);

            let attrs = CFDictionary::<CFType, CFType>::from_CFType_pairs(&[
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
                    // Permissive ACL baked in at creation — no SecKeychainItemSetAccess.
                    CFString::wrap_under_get_rule(kSecAttrAccess).as_CFType(),
                    cf_access.as_CFType(),
                ),
            ]);

            let status = SecItemAdd(attrs.as_concrete_TypeRef() as _, ptr::null_mut());
            if status == ERR_SEC_DUPLICATE_ITEM {
                // Delete refused (probably an ACL-protected legacy item in a
                // different keychain).  Update the value in place instead.
                return sec_item_update_value(service, account, secret);
            }
            if status != ERR_SEC_SUCCESS {
                return Err(Error::Vault(format!(
                    "keychain store failed (OSStatus {status})"
                )));
            }
            // cf_access drops → CFRelease(access). data, attrs drop normally.
        }
        Ok(())
    }

    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
        // Use SecItemCopyMatching (modern API) instead of get_generic_password
        // (legacy SecKeychainFindGenericPassword).  The legacy API checks a
        // per-binary ACL that triggers "Always Allow" prompts on every new
        // cargo build.  SecItemCopyMatching respects the kSecAttrAccess we
        // baked in at creation time — zero dialogs.
        let bytes = unsafe { retrieve_via_sec_item(service, account) }?;

        // Migration: if the item was stored with an old ACL (pre-permissive),
        // delete and re-create with the permissive ACL.  Uses the legacy API
        // for delete (which never prompts — no data access required).
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

    /// Silent check: does this item need one-time user authorization?
    ///
    /// Uses `kSecUseAuthenticationUI = "fail"` so macOS returns
    /// `errSecInteractionNotAllowed` instead of showing any dialog.
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

// ── ACL helper ────────────────────────────────────────────────────────────

/// Build a `SecAccess` whose trusted list contains the "any application"
/// sentinel.  Returns `Some(access)` on success (caller owns the +1 retain),
/// or `None` on failure.
///
/// # Retain-count notes
///
/// `SecTrustedApplicationCreateFromPath` returns RC=1 (create rule).
/// `CFType::wrap_under_create_rule` takes ownership without adding a retain.
/// `CFArray::from_CFTypes` calls `CFArrayCreate` which retains each element → RC=2.
/// The `CFType` temporary in the slice is dropped after the array is built → RC=1.
/// When `trusted` drops at function end, `CFArrayRelease` → RC=0 → any_app freed.
/// **Do NOT manually CFRelease(any_app)** — the array owns that retain.
///
/// `SecAccessCreate` returns RC=1 for `access` (create rule).
/// Caller receives that +1 and must CFRelease when done (or wrap in CFType).
unsafe fn make_any_app_access() -> Option<SecAccessRef> {
    let mut any_app: SecTrustedApplicationRef = ptr::null_mut();
    let s1 = unsafe { SecTrustedApplicationCreateFromPath(ptr::null(), &mut any_app) };
    if s1 != ERR_SEC_SUCCESS || any_app.is_null() {
        tracing::warn!("SecTrustedApplicationCreateFromPath failed: {s1}");
        return None;
    }

    // Wrap in CFType (takes ownership of RC=1).
    // CFArray retains it (+1 → RC=2); CFType temp drops (-1 → RC=1 held by array).
    let trusted: CFArray<CFType> =
        CFArray::from_CFTypes(&[unsafe { CFType::wrap_under_create_rule(any_app as CFTypeRef) }]);
    // DO NOT CFRelease(any_app) — trusted owns that retain.

    let label = CFString::new("contextcli credential");
    let mut access: SecAccessRef = ptr::null_mut();
    let s2 = unsafe {
        SecAccessCreate(
            label.as_concrete_TypeRef(),
            trusted.as_concrete_TypeRef() as _,
            &mut access,
        )
    };
    // trusted drops here → CFArrayRelease → any_app RC=0 → freed. Safe.

    if s2 != ERR_SEC_SUCCESS || access.is_null() {
        tracing::warn!("SecAccessCreate failed: {s2}");
        return None;
    }

    Some(access) // caller owns RC=1
}

// ── Legacy fallback ───────────────────────────────────────────────────────

/// Store using `SecKeychainAddGenericPassword` (no permissive ACL).
/// Used only when `SecAccessCreate` fails.  Items stored this way prompt
/// once per binary hash but never require a password.
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

// ── Modern retrieve ──────────────────────────────────────────────────────

/// Retrieve a keychain item using `SecItemCopyMatching` (modern API).
///
/// Unlike `SecKeychainFindGenericPassword`, this respects the `kSecAttrAccess`
/// ACL baked in at creation time.  Items stored with the "any application"
/// sentinel via `make_any_app_access()` are returned silently — no dialog,
/// no "Always Allow", no password prompt.
unsafe fn retrieve_via_sec_item(service: &str, account: &str) -> Result<Vec<u8>> {
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
            unsafe { CFString::wrap_under_get_rule(kSecReturnData) }.as_CFType(),
            CFBoolean::true_value().as_CFType(),
        ),
    ]);

    let mut result: CFTypeRef = ptr::null_mut();
    let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef() as _, &mut result) };

    if status == ERR_SEC_ITEM_NOT_FOUND {
        return Err(Error::Vault("keychain item not found".to_string()));
    }
    if status != ERR_SEC_SUCCESS {
        return Err(Error::Vault(format!(
            "keychain retrieve failed (OSStatus {status})"
        )));
    }
    if result.is_null() {
        return Err(Error::Vault("keychain returned null data".to_string()));
    }

    // result is a CFDataRef when kSecReturnData = true
    let cf_data = unsafe { CFData::wrap_under_create_rule(result as _) };
    Ok(cf_data.to_vec())
}

/// Delete every matching generic-password item via the modern API.
/// Metadata-only → no ACL check → no prompt.  Silently ignores "not found".
unsafe fn sec_item_delete(service: &str, account: &str) {
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
    ]);
    // Swallow all statuses — caller treats this as best-effort cleanup.
    let _ = unsafe { SecItemDelete(query.as_concrete_TypeRef() as _) };
}

/// In-place update of an existing item's secret data.  Used as a fallback
/// when add-with-delete-first still reports duplicate (e.g. legacy items with
/// ACLs that block our delete).  Does NOT attempt to rewrite the ACL.
unsafe fn sec_item_update_value(service: &str, account: &str, secret: &[u8]) -> Result<()> {
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
    ]);
    let data = CFData::from_buffer(secret);
    let updates = CFDictionary::<CFType, CFType>::from_CFType_pairs(&[(
        unsafe { CFString::wrap_under_get_rule(kSecValueData) }.as_CFType(),
        data.as_CFType(),
    )]);
    let status = unsafe {
        SecItemUpdate(
            query.as_concrete_TypeRef() as _,
            updates.as_concrete_TypeRef() as _,
        )
    };
    if status != ERR_SEC_SUCCESS {
        return Err(Error::Vault(format!(
            "keychain update failed (OSStatus {status})"
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
