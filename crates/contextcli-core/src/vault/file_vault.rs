//! File-based credential store for Linux and Windows.
//! Stores secrets as individual files in a protected directory.
//! Each file is named by a hash of service/account and contains the secret.
//!
//! Not as secure as OS keychain, but works cross-platform.
//! Directory permissions are set to 0700 (Unix) to restrict access.

use crate::error::{Error, Result};
use crate::vault::CredentialStore;
use std::path::PathBuf;

pub struct FileVault {
    dir: PathBuf,
}

impl FileVault {
    pub fn new(dir: PathBuf) -> Self {
        // Ensure vault directory exists with restricted permissions
        if let Err(e) = Self::ensure_dir(&dir) {
            tracing::warn!("failed to create vault dir: {e}");
        }
        Self { dir }
    }

    fn ensure_dir(dir: &PathBuf) -> Result<()> {
        std::fs::create_dir_all(dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }

        Ok(())
    }

    fn key_path(&self, service: &str, account: &str) -> PathBuf {
        // Simple hash to avoid filesystem-unfriendly characters
        let key = format!("{service}/{account}");
        let hash = simple_hash(&key);
        self.dir.join(hash)
    }
}

impl CredentialStore for FileVault {
    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()> {
        let path = self.key_path(service, account);
        // Store as: account_line\nsecret_bytes (so we can verify on retrieve)
        let mut data = format!("{service}/{account}\n").into_bytes();
        data.extend_from_slice(secret);
        std::fs::write(&path, &data)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
        let path = self.key_path(service, account);
        let data = std::fs::read(&path).map_err(|_| {
            Error::Vault(format!("secret not found: {service}/{account}"))
        })?;

        // Split at first newline: header\nsecret
        let newline_pos = data.iter().position(|&b| b == b'\n').ok_or_else(|| {
            Error::Vault("corrupt vault file".to_string())
        })?;

        Ok(data[newline_pos + 1..].to_vec())
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        let path = self.key_path(service, account);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

/// Simple deterministic hash for filenames (not cryptographic).
fn simple_hash(input: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_vault_round_trip() {
        let dir = std::env::temp_dir().join("contextcli-test-vault");
        let _ = std::fs::remove_dir_all(&dir);

        let vault = FileVault::new(dir.clone());
        let secret = b"my-secret-token";

        vault.store("test-svc", "test/account", secret).unwrap();
        let retrieved = vault.retrieve("test-svc", "test/account").unwrap();
        assert_eq!(retrieved, secret);

        vault.delete("test-svc", "test/account").unwrap();
        assert!(vault.retrieve("test-svc", "test/account").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
