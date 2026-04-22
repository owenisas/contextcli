pub mod adapter;
pub mod db;
pub mod error;
pub mod jwt;
pub mod profile;
pub mod project;
pub mod router;
pub mod vault;

use crate::adapter::AdapterRegistry;
use crate::adapter::types::AdapterContext;
use crate::error::Result;
use crate::profile::types::AuthState;
use crate::profile::ProfileManager;
use crate::router::Router;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Resolved data directory (~/.contextcli or CONTEXTCLI_HOME).
pub fn data_dir() -> PathBuf {
    if let Ok(home) = std::env::var("CONTEXTCLI_HOME") {
        return PathBuf::from(home);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".contextcli")
}

/// Top-level application context. Owns all shared state.
pub struct AppContext {
    pub registry: AdapterRegistry,
    pub profile_manager: ProfileManager,
    pub data_dir: PathBuf,
}

impl AppContext {
    /// Initialize with real macOS Keychain and SQLite.
    /// Loads adapters from ~/.contextcli/adapters.toml.
    /// Detects binaries and auto-imports existing native credentials.
    pub fn init() -> Result<Self> {
        let data_dir = data_dir();
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("contextcli.db");
        let conn = db::open_and_migrate(&db_path)?;
        let conn = Arc::new(Mutex::new(conn));

        let vault = vault::create_store(&data_dir);
        let profile_manager = ProfileManager::new(conn, vault);
        let registry = AdapterRegistry::from_config(&data_dir);

        // Register adapters, detect binaries, auto-import credentials
        for adapter in registry.list() {
            profile_manager.ensure_app(
                adapter.id(),
                adapter.display_name(),
                adapter.version(),
                adapter.support_level(),
            )?;

            // Detect binary path
            let binary_name = adapter.binary_name();
            let binary_found = if let Ok(path) = which::which(binary_name) {
                let _ =
                    profile_manager.update_binary_path(adapter.id(), &path.to_string_lossy());
                true
            } else {
                false
            };

            // Auto-import existing native credentials if binary found and no profiles yet
            if binary_found {
                let existing_profiles = profile_manager.list_profiles(adapter.id()).unwrap_or_default();
                if existing_profiles.is_empty() {
                    // Collect existing identities to deduplicate across import methods
                    let existing_identities: std::collections::HashSet<String> =
                        existing_profiles.iter()
                            .filter_map(|p| p.auth_user.clone())
                            .collect();

                    let config_dir = data_dir.join("configs").join(adapter.id()).join("default");
                    let ctx = AdapterContext {
                        config_dir,
                        profile_name: "default".to_string(),
                        app_id: adapter.id().to_string(),
                    };

                    match adapter.import_all_accounts(&ctx) {
                        Ok(accounts) if !accounts.is_empty() => {
                            let mut first = true;
                            let mut imported = 0usize;
                            for (profile_name, creds) in &accounts {
                                // Skip if a profile with the same identity already exists
                                if let Some(ref identity) = creds.identity {
                                    if existing_identities.contains(identity) {
                                        tracing::debug!(
                                            "skipping duplicate {} account: {}",
                                            adapter.display_name(),
                                            identity
                                        );
                                        continue;
                                    }
                                }
                                // Skip if a profile with the same name already exists
                                if profile_manager.profile_exists(adapter.id(), profile_name) {
                                    continue;
                                }

                                if let Ok(profile) = profile_manager.create_profile(
                                    adapter.id(),
                                    profile_name,
                                    Some("Auto-imported"),
                                ) {
                                    for (key, value) in &creds.fields {
                                        let _ =
                                            profile_manager.store_secret(&profile, key, value);
                                    }
                                    let _ = profile_manager.update_auth_state(
                                        adapter.id(),
                                        profile_name,
                                        AuthState::Authenticated,
                                        creds.identity.as_deref(),
                                    );
                                    // First profile becomes default
                                    if first {
                                        let _ = profile_manager
                                            .set_default(adapter.id(), profile_name);
                                        first = false;
                                    }
                                    let _ = profile_manager.log_activity(
                                        adapter.id(),
                                        Some(&profile.id),
                                        "auto_import",
                                        creds.identity.as_deref(),
                                    );
                                    imported += 1;
                                }
                            }
                            if imported > 0 {
                                tracing::info!(
                                    "auto-imported {} {} account(s)",
                                    imported,
                                    adapter.display_name()
                                );
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!(
                                "failed to auto-import {} credentials: {e}",
                                adapter.display_name()
                            );
                        }
                    }
                }
            }
        }

        Ok(Self {
            registry,
            profile_manager,
            data_dir,
        })
    }

    /// Create a Router referencing this context.
    pub fn router(&self) -> Router<'_> {
        Router {
            registry: &self.registry,
            profile_manager: &self.profile_manager,
            data_dir: self.data_dir.clone(),
        }
    }
}
