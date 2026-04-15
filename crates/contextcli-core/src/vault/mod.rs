pub mod keychain;

use crate::error::Result;

/// Abstraction over OS credential storage.
/// macOS: Keychain. Future: Windows Credential Manager, Linux libsecret.
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
}

/// Key naming convention for ContextCLI secrets.
/// Service: "contextcli"
/// Account: "<app_id>/<profile_name>/<field>" e.g. "vercel/work/token"
pub const VAULT_SERVICE: &str = "contextcli";

pub fn vault_account(app_id: &str, profile_name: &str, field: &str) -> String {
    format!("{app_id}/{profile_name}/{field}")
}
