#[cfg(target_os = "macos")]
pub mod keychain;

pub mod file_vault;

use crate::error::Result;

/// Abstraction over OS credential storage.
/// macOS: Keychain. Linux/Windows: encrypted file vault.
pub trait CredentialStore: Send + Sync {
    /// Store a secret value.
    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()>;

    /// Retrieve a secret value.
    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>>;

    /// Delete a secret.
    fn delete(&self, service: &str, account: &str) -> Result<()>;

    /// Check if a secret exists without retrieving it.
    fn exists(&self, service: &str, account: &str) -> bool {
        self.retrieve(service, account).is_ok()
    }

    /// Returns true if the secret exists but requires one-time user authorization
    /// before it can be read silently (macOS Keychain ACL migration needed).
    /// Always false on non-macOS platforms.
    fn needs_auth(&self, _service: &str, _account: &str) -> bool {
        false
    }
}

/// Key naming convention for ContextCLI secrets.
/// Service: "contextcli"
/// Account: "<app_id>/<profile_name>/<field>" e.g. "vercel/work/token"
pub const VAULT_SERVICE: &str = "contextcli";

pub fn vault_account(app_id: &str, profile_name: &str, field: &str) -> String {
    format!("{app_id}/{profile_name}/{field}")
}

/// Create the platform-appropriate credential store.
///
/// On macOS the default is the Keychain. Set `CONTEXTCLI_VAULT=file` to force
/// the file-based encrypted vault — useful for dev builds and users who don't
/// want the rebuild-triggered keychain prompts. `CONTEXTCLI_VAULT=keychain` is
/// also accepted for explicitness (macOS only; ignored elsewhere).
pub fn create_store(data_dir: &std::path::Path) -> Box<dyn CredentialStore> {
    let backend = std::env::var("CONTEXTCLI_VAULT")
        .ok()
        .map(|s| s.to_ascii_lowercase());

    match backend.as_deref() {
        Some("file") => Box::new(file_vault::FileVault::new(data_dir.join("vault"))),
        #[cfg(target_os = "macos")]
        Some("keychain") | None => Box::new(keychain::KeychainStore::new()),
        #[cfg(not(target_os = "macos"))]
        _ => Box::new(file_vault::FileVault::new(data_dir.join("vault"))),
        #[cfg(target_os = "macos")]
        Some(other) => {
            tracing::warn!(
                "unknown CONTEXTCLI_VAULT={other:?}; falling back to keychain"
            );
            Box::new(keychain::KeychainStore::new())
        }
    }
}
