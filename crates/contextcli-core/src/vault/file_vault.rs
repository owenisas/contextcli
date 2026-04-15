//! File-based credential store for Linux and Windows.
//! Stores secrets AES-256-GCM encrypted in individual files in a protected directory.
//! A random 32-byte key is generated on first use and stored at {dir}/.key (mode 0600).
//! On macOS the system Keychain is preferred; this backend is for Linux/Windows.

use crate::error::{Error, Result};
use crate::vault::CredentialStore;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Magic prefix for v1 encrypted vault files.
const MAGIC: [u8; 4] = *b"CV1\x00";

pub struct FileVault {
    dir: PathBuf,
}

impl FileVault {
    pub fn new(dir: PathBuf) -> Self {
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

    /// Load the AES-256 key from disk, generating and saving it on first use.
    fn load_or_create_key(&self) -> Result<[u8; 32]> {
        let key_path = self.dir.join(".key");
        if key_path.exists() {
            let bytes = std::fs::read(&key_path)?;
            if bytes.len() != 32 {
                return Err(Error::Vault("vault key file has wrong length".to_string()));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            Ok(key)
        } else {
            use rand::RngCore;
            let mut key = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            std::fs::write(&key_path, &key)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    &key_path,
                    std::fs::Permissions::from_mode(0o600),
                )?;
            }
            Ok(key)
        }
    }

    /// Derive the vault file path for a given service/account pair using SHA-256.
    fn key_path(&self, service: &str, account: &str) -> PathBuf {
        let input = format!("{service}/{account}");
        let hash = hex::encode(Sha256::digest(input.as_bytes()));
        self.dir.join(hash)
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key_bytes = self.load_or_create_key()?;
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        // Random 12-byte nonce per encryption
        use rand::RngCore;
        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| Error::Vault("encryption failed".to_string()))?;

        // Layout: MAGIC(4) | nonce(12) | ciphertext+tag
        let mut out = Vec::with_capacity(4 + 12 + ciphertext.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Detect legacy plaintext (no magic header)
        if data.len() < 4 || data[..4] != MAGIC {
            // Legacy format: "service/account\nsecret"
            let newline = data.iter().position(|&b| b == b'\n').ok_or_else(|| {
                Error::Vault("corrupt legacy vault file".to_string())
            })?;
            return Ok(data[newline + 1..].to_vec());
        }

        if data.len() < 16 {
            return Err(Error::Vault("encrypted vault file too short".to_string()));
        }

        let key_bytes = self.load_or_create_key()?;
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&data[4..16]);
        let ciphertext = &data[16..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| Error::Vault("decryption failed — vault key may have changed".to_string()))
    }
}

impl CredentialStore for FileVault {
    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()> {
        let path = self.key_path(service, account);
        let encrypted = self.encrypt(secret)?;
        std::fs::write(&path, &encrypted)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
        let path = self.key_path(service, account);
        let data = std::fs::read(&path)
            .map_err(|_| Error::Vault(format!("secret not found: {service}/{account}")))?;
        self.decrypt(&data)
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        let path = self.key_path(service, account);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_vault_round_trip_encrypted() {
        let dir = std::env::temp_dir().join("contextcli-test-vault-enc");
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

    #[test]
    fn test_file_vault_legacy_migration() {
        let dir = std::env::temp_dir().join("contextcli-test-vault-legacy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let vault = FileVault::new(dir.clone());
        // Write a legacy plaintext file directly
        let legacy_path = vault.key_path("test-svc", "my-account");
        let legacy_data = b"test-svc/my-account\nmy-legacy-secret";
        std::fs::write(&legacy_path, legacy_data).unwrap();

        let retrieved = vault.retrieve("test-svc", "my-account").unwrap();
        assert_eq!(retrieved, b"my-legacy-secret");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sha256_filename_uniqueness() {
        let dir = std::env::temp_dir().join("contextcli-test-vault-hash");
        let vault = FileVault::new(dir);
        let h1 = vault.key_path("svc", "account1");
        let h2 = vault.key_path("svc", "account2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_file_vault_overwrite() {
        let dir = std::env::temp_dir().join("contextcli-test-vault-overwrite");
        let _ = std::fs::remove_dir_all(&dir);

        let vault = FileVault::new(dir.clone());
        vault.store("svc", "acct", b"first").unwrap();
        vault.store("svc", "acct", b"second").unwrap();
        let got = vault.retrieve("svc", "acct").unwrap();
        assert_eq!(got, b"second");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
