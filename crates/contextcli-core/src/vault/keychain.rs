use crate::error::{Error, Result};
use crate::vault::CredentialStore;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

/// macOS Keychain credential store using Security.framework.
pub struct KeychainStore;

impl KeychainStore {
    pub fn new() -> Self {
        Self
    }
}

impl CredentialStore for KeychainStore {
    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()> {
        // Delete existing entry first (set_generic_password fails if it already exists)
        let _ = delete_generic_password(service, account);
        set_generic_password(service, account, secret)
            .map_err(|e| Error::Vault(format!("keychain store failed: {e}")))?;
        Ok(())
    }

    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
        get_generic_password(service, account)
            .map_err(|e| Error::Vault(format!("keychain retrieve failed: {e}")))
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        delete_generic_password(service, account)
            .map_err(|e| Error::Vault(format!("keychain delete failed: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: these tests interact with the real macOS Keychain.
    // They use a unique service name to avoid collisions.
    const TEST_SERVICE: &str = "contextcli-test-ephemeral";
    const TEST_ACCOUNT: &str = "test/unit/token";

    #[test]
    fn test_store_retrieve_delete() {
        let store = KeychainStore::new();
        let secret = b"test-secret-value-12345";

        // Store
        store.store(TEST_SERVICE, TEST_ACCOUNT, secret).unwrap();

        // Retrieve
        let retrieved = store.retrieve(TEST_SERVICE, TEST_ACCOUNT).unwrap();
        assert_eq!(retrieved, secret);

        // Overwrite
        let new_secret = b"updated-secret";
        store.store(TEST_SERVICE, TEST_ACCOUNT, new_secret).unwrap();
        let retrieved = store.retrieve(TEST_SERVICE, TEST_ACCOUNT).unwrap();
        assert_eq!(retrieved, new_secret);

        // Delete
        store.delete(TEST_SERVICE, TEST_ACCOUNT).unwrap();

        // Should fail after delete
        assert!(store.retrieve(TEST_SERVICE, TEST_ACCOUNT).is_err());
    }

    #[test]
    fn test_retrieve_nonexistent() {
        let store = KeychainStore::new();
        assert!(store.retrieve(TEST_SERVICE, "nonexistent/key/here").is_err());
    }

    #[test]
    fn test_exists() {
        let store = KeychainStore::new();
        let account = "test/exists/token";

        assert!(!store.exists(TEST_SERVICE, account));
        store.store(TEST_SERVICE, account, b"val").unwrap();
        assert!(store.exists(TEST_SERVICE, account));
        store.delete(TEST_SERVICE, account).unwrap();
        assert!(!store.exists(TEST_SERVICE, account));
    }
}
