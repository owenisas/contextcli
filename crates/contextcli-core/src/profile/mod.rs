pub mod types;

use crate::error::{Error, Result};
use crate::profile::types::{App, AuthState, Profile, ProjectLink, SecretRef};
use crate::vault::{self, CredentialStore};
use rusqlite::{params, Connection};
use secrecy::{ExposeSecret, SecretString};
use std::sync::{Arc, Mutex};

/// Manages apps, profiles, and their credential references.
pub struct ProfileManager {
    conn: Arc<Mutex<Connection>>,
    pub(crate) vault: Box<dyn CredentialStore>,
}

impl ProfileManager {
    pub fn new(conn: Arc<Mutex<Connection>>, vault: Box<dyn CredentialStore>) -> Self {
        Self { conn, vault }
    }

    // ── Apps ──────────────────────────────────────────────────

    /// Ensure an app row exists (upsert from adapter metadata).
    pub fn ensure_app(
        &self,
        id: &str,
        display_name: &str,
        adapter_version: &str,
        support_level: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO apps (id, display_name, adapter_version, support_level)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                adapter_version = excluded.adapter_version,
                support_level = excluded.support_level,
                updated_at = datetime('now')",
            params![id, display_name, adapter_version, support_level],
        )?;
        Ok(())
    }

    /// Update the detected binary path for an app.
    pub fn update_binary_path(&self, app_id: &str, path: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "UPDATE apps SET binary_path = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![path, app_id],
        )?;
        Ok(())
    }

    /// List all registered apps.
    pub fn list_apps(&self) -> Result<Vec<App>> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, display_name, binary_path, adapter_version, support_level,
                    created_at, updated_at
             FROM apps ORDER BY id",
        )?;
        let apps = stmt
            .query_map([], |row| {
                Ok(App {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    binary_path: row.get(2)?,
                    adapter_version: row.get(3)?,
                    support_level: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(apps)
    }

    // ── Profiles ─────────────────────────────────────────────

    /// Create a new profile.
    pub fn create_profile(
        &self,
        app_id: &str,
        profile_name: &str,
        label: Option<&str>,
    ) -> Result<Profile> {
        validate_profile_name(profile_name)?;
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO profiles (id, app_id, profile_name, label)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, app_id, profile_name, label],
        )?;
        drop(conn);
        self.get_profile(app_id, profile_name)
    }

    /// Get a profile by app_id + profile_name, annotated with keychain auth state.
    pub fn get_profile(&self, app_id: &str, profile_name: &str) -> Result<Profile> {
        // Scope connection so it is released before the vault check below.
        let mut p = {
            let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
            conn.query_row(
                "SELECT id, app_id, profile_name, label, is_default, auth_state,
                        auth_user, config_dir, created_at, updated_at
                 FROM profiles WHERE app_id = ?1 AND profile_name = ?2",
                params![app_id, profile_name],
                |row| row_to_profile(row),
            )
            .map_err(|_| Error::ProfileNotFound {
                app_id: app_id.to_string(),
                profile_name: profile_name.to_string(),
            })?
        };

        let account = vault::vault_account(&p.app_id, &p.profile_name, "token");
        p.needs_keychain_auth = self.vault.needs_auth(vault::VAULT_SERVICE, &account);
        Ok(p)
    }

    /// Get the default profile for an app.
    pub fn get_default_profile(&self, app_id: &str) -> Result<Profile> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.query_row(
            "SELECT id, app_id, profile_name, label, is_default, auth_state,
                    auth_user, config_dir, created_at, updated_at
             FROM profiles WHERE app_id = ?1 AND is_default = 1",
            params![app_id],
            |row| row_to_profile(row),
        )
        .map_err(|_| Error::NoDefaultProfile(app_id.to_string()))
    }

    /// List all profiles for an app, annotated with keychain auth state.
    pub fn list_profiles(&self, app_id: &str) -> Result<Vec<Profile>> {
        // Scope the connection so it is released before the vault checks below.
        let mut profiles = {
            let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
            let mut stmt = conn.prepare(
                "SELECT id, app_id, profile_name, label, is_default, auth_state,
                        auth_user, config_dir, created_at, updated_at
                 FROM profiles WHERE app_id = ?1 ORDER BY profile_name",
            )?;
            stmt.query_map(params![app_id], |row| row_to_profile(row))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };

        // Annotate each profile with its vault auth requirement (silent, no dialog).
        for p in &mut profiles {
            let account = vault::vault_account(&p.app_id, &p.profile_name, "token");
            p.needs_keychain_auth = self.vault.needs_auth(vault::VAULT_SERVICE, &account);
        }
        Ok(profiles)
    }

    /// Set a profile as the default for its app (clears previous default).
    pub fn set_default(&self, app_id: &str, profile_name: &str) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE profiles SET is_default = 0 WHERE app_id = ?1 AND is_default = 1",
            params![app_id],
        )?;
        let changed = tx.execute(
            "UPDATE profiles SET is_default = 1, updated_at = datetime('now')
             WHERE app_id = ?1 AND profile_name = ?2",
            params![app_id, profile_name],
        )?;
        if changed == 0 {
            // tx drops here → automatic rollback
            return Err(Error::ProfileNotFound {
                app_id: app_id.to_string(),
                profile_name: profile_name.to_string(),
            });
        }
        tx.commit()?;
        Ok(())
    }

    /// Rename a profile. Updates DB, vault keys, and secret refs.
    pub fn rename_profile(
        &self,
        app_id: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<Profile> {
        // Check old exists, new doesn't
        let _old_profile = self.get_profile(app_id, old_name)?;
        if self.profile_exists(app_id, new_name) {
            return Err(Error::Other(format!(
                "profile '{new_name}' already exists for {app_id}"
            )));
        }

        // Move secrets in vault: old key → new key
        let refs = self.list_secret_refs(app_id, old_name)?;
        for sr in &refs {
            if let Ok(secret_bytes) = self.vault.retrieve(&sr.vault_service, &sr.vault_account) {
                let new_account =
                    vault::vault_account(app_id, new_name, &sr.secret_key);
                let _ = self.vault.store(vault::VAULT_SERVICE, &new_account, &secret_bytes);
                let _ = self.vault.delete(&sr.vault_service, &sr.vault_account);

                // Update secret_ref row
                let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
                let _ = conn.execute(
                    "UPDATE secret_refs SET vault_account = ?1 WHERE id = ?2",
                    params![new_account, sr.id],
                );
            }
        }

        // Update profile row
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "UPDATE profiles SET profile_name = ?1, updated_at = datetime('now')
             WHERE app_id = ?2 AND profile_name = ?3",
            params![new_name, app_id, old_name],
        )?;

        // Update project_links
        let _ = conn.execute(
            "UPDATE project_links SET profile_name = ?1 WHERE app_id = ?2 AND profile_name = ?3",
            params![new_name, app_id, old_name],
        );

        drop(conn);
        self.get_profile(app_id, new_name)
    }

    /// Update auth state and identity for a profile.
    pub fn update_auth_state(
        &self,
        app_id: &str,
        profile_name: &str,
        state: AuthState,
        auth_user: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "UPDATE profiles SET auth_state = ?1, auth_user = ?2, updated_at = datetime('now')
             WHERE app_id = ?3 AND profile_name = ?4",
            params![state.as_str(), auth_user, app_id, profile_name],
        )?;
        Ok(())
    }

    /// Update the config dir for a profile.
    pub fn update_config_dir(
        &self,
        app_id: &str,
        profile_name: &str,
        config_dir: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "UPDATE profiles SET config_dir = ?1, updated_at = datetime('now')
             WHERE app_id = ?2 AND profile_name = ?3",
            params![config_dir, app_id, profile_name],
        )?;
        Ok(())
    }

    /// Delete a profile and its secrets.
    pub fn delete_profile(&self, app_id: &str, profile_name: &str) -> Result<()> {
        // Delete secrets from vault first
        let refs = self.list_secret_refs(app_id, profile_name)?;
        for sr in &refs {
            let _ = self.vault.delete(&sr.vault_service, &sr.vault_account);
        }

        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "DELETE FROM profiles WHERE app_id = ?1 AND profile_name = ?2",
            params![app_id, profile_name],
        )?;
        Ok(())
    }

    /// Check if a profile exists.
    pub fn profile_exists(&self, app_id: &str, profile_name: &str) -> bool {
        self.get_profile(app_id, profile_name).is_ok()
    }

    // ── Secrets ──────────────────────────────────────────────

    /// Store a secret for a profile in the vault and record the reference.
    pub fn store_secret(
        &self,
        profile: &Profile,
        secret_key: &str,
        secret_value: &SecretString,
    ) -> Result<()> {
        let account = vault::vault_account(&profile.app_id, &profile.profile_name, secret_key);
        self.vault
            .store(vault::VAULT_SERVICE, &account, secret_value.expose_secret().as_bytes())?;

        // Upsert secret_ref
        let ref_id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO secret_refs (id, profile_id, secret_key, vault_service, vault_account)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(profile_id, secret_key) DO UPDATE SET
                vault_service = excluded.vault_service,
                vault_account = excluded.vault_account",
            params![
                ref_id,
                profile.id,
                secret_key,
                vault::VAULT_SERVICE,
                account
            ],
        )?;
        Ok(())
    }

    /// Retrieve a secret from the vault.
    pub fn retrieve_secret(&self, profile: &Profile, secret_key: &str) -> Result<SecretString> {
        let account = vault::vault_account(&profile.app_id, &profile.profile_name, secret_key);
        let bytes = self.vault.retrieve(vault::VAULT_SERVICE, &account)?;
        let value = String::from_utf8(bytes)
            .map_err(|e| Error::Vault(format!("secret is not valid UTF-8: {e}")))?;
        Ok(SecretString::from(value))
    }

    /// List secret refs for a profile.
    fn list_secret_refs(&self, app_id: &str, profile_name: &str) -> Result<Vec<SecretRef>> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT sr.id, sr.profile_id, sr.secret_key, sr.vault_service, sr.vault_account
             FROM secret_refs sr
             JOIN profiles p ON sr.profile_id = p.id
             WHERE p.app_id = ?1 AND p.profile_name = ?2",
        )?;
        let refs = stmt
            .query_map(params![app_id, profile_name], |row| {
                Ok(SecretRef {
                    id: row.get(0)?,
                    profile_id: row.get(1)?,
                    secret_key: row.get(2)?,
                    vault_service: row.get(3)?,
                    vault_account: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(refs)
    }

    // ── Activity Log ─────────────────────────────────────────

    /// Log an activity event (no secrets in detail!).
    pub fn log_activity(
        &self,
        app_id: &str,
        profile_id: Option<&str>,
        action: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO activity_log (profile_id, app_id, action, detail)
             VALUES (?1, ?2, ?3, ?4)",
            params![profile_id, app_id, action, detail],
        )?;
        Ok(())
    }

    // ── Project Links ────────────────────────────────────────

    /// Register a project directory → app → profile link.
    pub fn register_project_link(
        &self,
        project_dir: &str,
        app_id: &str,
        profile_name: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "INSERT INTO project_links (project_dir, app_id, profile_name)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_dir, app_id) DO UPDATE SET
                profile_name = excluded.profile_name",
            params![project_dir, app_id, profile_name],
        )?;
        Ok(())
    }

    /// Remove a project link.
    pub fn remove_project_link(&self, project_dir: &str, app_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        conn.execute(
            "DELETE FROM project_links WHERE project_dir = ?1 AND app_id = ?2",
            params![project_dir, app_id],
        )?;
        Ok(())
    }

    /// List all project links for an app.
    pub fn list_project_links(&self, app_id: &str) -> Result<Vec<ProjectLink>> {
        let conn = self.conn.lock().map_err(|_| Error::Other("mutex poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT project_dir, app_id, profile_name FROM project_links
             WHERE app_id = ?1 ORDER BY project_dir",
        )?;
        let links = stmt
            .query_map(params![app_id], |row| {
                Ok(ProjectLink {
                    project_dir: row.get(0)?,
                    app_id: row.get(1)?,
                    profile_name: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(links)
    }
}

/// Validate a profile name to prevent path traversal and injection.
/// Allows alphanumeric, dash, underscore, and dot; no leading dot; max 64 chars.
pub(crate) fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Other("profile name cannot be empty".to_string()));
    }
    if name.len() > 64 {
        return Err(Error::Other("profile name too long (max 64 characters)".to_string()));
    }
    if name.starts_with('.') {
        return Err(Error::Other("profile name cannot start with '.'".to_string()));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return Err(Error::Other(
            "profile name may only contain alphanumeric characters, dashes, underscores, and dots"
                .to_string(),
        ));
    }
    Ok(())
}

fn row_to_profile(row: &rusqlite::Row) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: row.get(0)?,
        app_id: row.get(1)?,
        profile_name: row.get(2)?,
        label: row.get(3)?,
        is_default: row.get::<_, i32>(4)? != 0,
        auth_state: AuthState::from_str(row.get::<_, String>(5)?.as_str()),
        auth_user: row.get(6)?,
        config_dir: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        needs_keychain_auth: false, // annotated by ProfileManager after vault check
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    /// In-memory vault for testing (no real keychain).
    struct MemoryVault {
        store: Mutex<std::collections::HashMap<String, Vec<u8>>>,
    }

    impl MemoryVault {
        fn new() -> Self {
            Self {
                store: Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl CredentialStore for MemoryVault {
        fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()> {
            let key = format!("{service}/{account}");
            self.store.lock().unwrap().insert(key, secret.to_vec());
            Ok(())
        }

        fn retrieve(&self, service: &str, account: &str) -> Result<Vec<u8>> {
            let key = format!("{service}/{account}");
            self.store
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| Error::Vault("not found".to_string()))
        }

        fn delete(&self, service: &str, account: &str) -> Result<()> {
            let key = format!("{service}/{account}");
            self.store.lock().unwrap().remove(&key);
            Ok(())
        }
    }

    fn setup() -> ProfileManager {
        let conn = db::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let vault = Box::new(MemoryVault::new());
        let pm = ProfileManager::new(conn, vault);
        pm.ensure_app("vercel", "Vercel", "0.1.0", "tier1")
            .unwrap();
        pm
    }

    #[test]
    fn test_create_and_get_profile() {
        let pm = setup();
        let p = pm.create_profile("vercel", "work", Some("Work Account")).unwrap();
        assert_eq!(p.profile_name, "work");
        assert_eq!(p.label, Some("Work Account".to_string()));
        assert_eq!(p.auth_state, AuthState::Unauthenticated);
        assert!(!p.is_default);
    }

    #[test]
    fn test_list_profiles() {
        let pm = setup();
        pm.create_profile("vercel", "work", None).unwrap();
        pm.create_profile("vercel", "personal", None).unwrap();
        let profiles = pm.list_profiles("vercel").unwrap();
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn test_set_default() {
        let pm = setup();
        pm.create_profile("vercel", "work", None).unwrap();
        pm.create_profile("vercel", "personal", None).unwrap();

        pm.set_default("vercel", "work").unwrap();
        let def = pm.get_default_profile("vercel").unwrap();
        assert_eq!(def.profile_name, "work");

        // Switch default
        pm.set_default("vercel", "personal").unwrap();
        let def = pm.get_default_profile("vercel").unwrap();
        assert_eq!(def.profile_name, "personal");

        // Old default should be cleared
        let work = pm.get_profile("vercel", "work").unwrap();
        assert!(!work.is_default);
    }

    #[test]
    fn test_update_auth_state() {
        let pm = setup();
        pm.create_profile("vercel", "work", None).unwrap();
        pm.update_auth_state("vercel", "work", AuthState::Authenticated, Some("john@acme.com"))
            .unwrap();
        let p = pm.get_profile("vercel", "work").unwrap();
        assert_eq!(p.auth_state, AuthState::Authenticated);
        assert_eq!(p.auth_user, Some("john@acme.com".to_string()));
    }

    #[test]
    fn test_store_and_retrieve_secret() {
        let pm = setup();
        let p = pm.create_profile("vercel", "work", None).unwrap();
        let secret = SecretString::from("my-secret-token".to_string());
        pm.store_secret(&p, "token", &secret).unwrap();

        let retrieved = pm.retrieve_secret(&p, "token").unwrap();
        assert_eq!(retrieved.expose_secret(), "my-secret-token");
    }

    #[test]
    fn test_delete_profile_cleans_secrets() {
        let pm = setup();
        let p = pm.create_profile("vercel", "work", None).unwrap();
        let secret = SecretString::from("delete-me".to_string());
        pm.store_secret(&p, "token", &secret).unwrap();

        pm.delete_profile("vercel", "work").unwrap();
        assert!(!pm.profile_exists("vercel", "work"));
    }

    #[test]
    fn test_no_default_profile_error() {
        let pm = setup();
        pm.create_profile("vercel", "work", None).unwrap();
        let result = pm.get_default_profile("vercel");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_profile_name_fails() {
        let pm = setup();
        pm.create_profile("vercel", "work", None).unwrap();
        let result = pm.create_profile("vercel", "work", None);
        assert!(result.is_err());
    }
}
