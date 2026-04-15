pub mod launcher;

use crate::adapter::types::{AdapterContext, ResolvedProfile};
use crate::adapter::AdapterRegistry;
use crate::error::{Error, Result};
use crate::profile::types::AuthState;
use crate::profile::ProfileManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitStatus;

/// Core router: resolve profile → prepare auth context → forward to native CLI.
pub struct Router<'a> {
    pub registry: &'a AdapterRegistry,
    pub profile_manager: &'a ProfileManager,
    pub data_dir: PathBuf,
}

impl<'a> Router<'a> {
    /// Forward a command to the native CLI under a specific profile.
    /// Checks .contextcli.toml in current directory for auto-profile and policies.
    pub fn forward(
        &self,
        app_id: &str,
        profile_name: Option<&str>,
        forward_args: &[String],
    ) -> Result<ExitStatus> {
        // 1. Look up adapter
        let adapter = self.registry.get(app_id)?;

        // 2. Check project config for auto-profile mapping
        let project_config = std::env::current_dir()
            .ok()
            .and_then(|cwd| crate::project::find_project_config(&cwd))
            .map(|(config, path)| {
                tracing::debug!("using project config: {}", path.display());
                config
            });

        // 3. Resolve profile: explicit flag > project config > default
        let resolved_profile_name = match profile_name {
            Some(name) => name.to_string(),
            None => {
                if let Some(ref pc) = project_config {
                    if let Some(mapped) = pc.profile_for(app_id) {
                        tracing::info!("auto-selected profile '{}' from .contextcli.toml", mapped);
                        mapped.to_string()
                    } else {
                        self.profile_manager
                            .get_default_profile(app_id)?
                            .profile_name
                    }
                } else {
                    self.profile_manager
                        .get_default_profile(app_id)?
                        .profile_name
                }
            }
        };

        let profile = self
            .profile_manager
            .get_profile(app_id, &resolved_profile_name)?;

        // 4. Enforce project policies
        if let Some(ref pc) = project_config {
            if let Some(reason) = pc.check_policy(app_id, &profile.profile_name, forward_args) {
                return Err(Error::Other(reason));
            }
        }

        // 5. Verify authenticated
        if profile.auth_state != AuthState::Authenticated {
            return Err(Error::NotAuthenticated {
                app_id: app_id.to_string(),
                profile_name: profile.profile_name.clone(),
            });
        }

        // 6. Hydrate secrets from vault
        let mut secrets = HashMap::new();
        for field in adapter.credential_fields() {
            if field.sensitive {
                let secret = self.profile_manager.retrieve_secret(&profile, &field.name)?;
                secrets.insert(field.name.clone(), secret);
            }
        }
        let resolved = ResolvedProfile {
            app_id: app_id.to_string(),
            profile_name: profile.profile_name.clone(),
            secrets,
        };

        // 7. Ask adapter to prepare invocation env
        let invocation = adapter.prepare_env(&resolved)?;

        // 8. Find binary
        let binary_name = adapter.binary_name();
        let binary = which::which(binary_name)
            .map_err(|_| Error::BinaryNotFound(binary_name.to_string()))?;

        // 9. Launch
        let status = launcher::spawn(&binary, &invocation, forward_args)?;

        // 10. Log activity (no secrets)
        let _ = self.profile_manager.log_activity(
            app_id,
            Some(&profile.id),
            "forward",
            Some(&format!("exit_code={}", status.code().unwrap_or(-1))),
        );

        Ok(status)
    }

    /// Perform login for a profile.
    pub fn login(&self, app_id: &str, profile_name: &str) -> Result<()> {
        crate::profile::validate_profile_name(profile_name)?;
        let adapter = self.registry.get(app_id)?;

        // Ensure app is registered
        self.profile_manager.ensure_app(
            adapter.id(),
            adapter.display_name(),
            adapter.version(),
            adapter.support_level(),
        )?;

        // Create profile if it doesn't exist
        if !self.profile_manager.profile_exists(app_id, profile_name) {
            self.profile_manager
                .create_profile(app_id, profile_name, None)?;
        }

        // Prepare adapter context with isolated config dir
        let config_dir = self
            .data_dir
            .join("configs")
            .join(app_id)
            .join(profile_name);
        let ctx = AdapterContext {
            config_dir: config_dir.clone(),
            profile_name: profile_name.to_string(),
            app_id: app_id.to_string(),
        };

        // Update config dir in DB
        self.profile_manager.update_config_dir(
            app_id,
            profile_name,
            &config_dir.to_string_lossy(),
        )?;

        // Run adapter login
        let creds = adapter.login(&ctx)?;

        // Store secrets
        let profile = self.profile_manager.get_profile(app_id, profile_name)?;
        for (key, value) in &creds.fields {
            self.profile_manager.store_secret(&profile, key, value)?;
        }

        // Update auth state
        self.profile_manager.update_auth_state(
            app_id,
            profile_name,
            AuthState::Authenticated,
            creds.identity.as_deref(),
        )?;

        // Log
        let _ = self.profile_manager.log_activity(
            app_id,
            Some(&profile.id),
            "login",
            creds
                .identity
                .as_ref()
                .map(|i| format!("identity={i}"))
                .as_deref(),
        );

        Ok(())
    }

    /// Import existing credentials from the native CLI's config.
    /// Creates a profile and stores the found credentials.
    pub fn import(&self, app_id: &str, profile_name: &str) -> Result<bool> {
        crate::profile::validate_profile_name(profile_name)?;
        let adapter = self.registry.get(app_id)?;

        // Ensure app registered
        self.profile_manager.ensure_app(
            adapter.id(),
            adapter.display_name(),
            adapter.version(),
            adapter.support_level(),
        )?;

        // Try importing
        let creds = match adapter.import_existing()? {
            Some(c) => c,
            None => return Ok(false),
        };

        // Create profile if needed
        if !self.profile_manager.profile_exists(app_id, profile_name) {
            self.profile_manager
                .create_profile(app_id, profile_name, None)?;
        }

        // Store secrets
        let profile = self.profile_manager.get_profile(app_id, profile_name)?;
        for (key, value) in &creds.fields {
            self.profile_manager.store_secret(&profile, key, value)?;
        }

        // Credentials found = Authenticated
        self.profile_manager.update_auth_state(
            app_id,
            profile_name,
            AuthState::Authenticated,
            creds.identity.as_deref(),
        )?;

        let _ = self.profile_manager.log_activity(
            app_id,
            Some(&profile.id),
            "import",
            creds
                .identity
                .as_ref()
                .map(|i| format!("identity={i}"))
                .as_deref(),
        );

        Ok(true)
    }

    /// Logout: clear credentials and update state.
    pub fn logout(&self, app_id: &str, profile_name: &str) -> Result<()> {
        let profile = self.profile_manager.get_profile(app_id, profile_name)?;

        // Clear secrets from vault
        let adapter = self.registry.get(app_id)?;
        for field in adapter.credential_fields() {
            if field.sensitive {
                let account = crate::vault::vault_account(app_id, profile_name, &field.name);
                let _ = self
                    .profile_manager
                    .vault
                    .delete(crate::vault::VAULT_SERVICE, &account);
            }
        }

        // Update state
        self.profile_manager.update_auth_state(
            app_id,
            profile_name,
            AuthState::Unauthenticated,
            None,
        )?;

        let _ = self
            .profile_manager
            .log_activity(app_id, Some(&profile.id), "logout", None);

        Ok(())
    }

    /// Validate a profile's auth state.
    pub fn validate(
        &self,
        app_id: &str,
        profile_name: &str,
    ) -> Result<crate::adapter::types::ValidationResult> {
        let adapter = self.registry.get(app_id)?;
        let profile = self.profile_manager.get_profile(app_id, profile_name)?;

        let config_dir = self
            .data_dir
            .join("configs")
            .join(app_id)
            .join(profile_name);
        let ctx = AdapterContext {
            config_dir,
            profile_name: profile_name.to_string(),
            app_id: app_id.to_string(),
        };

        // Hydrate secrets
        let mut secrets = HashMap::new();
        for field in adapter.credential_fields() {
            if field.sensitive {
                match self.profile_manager.retrieve_secret(&profile, &field.name) {
                    Ok(secret) => {
                        secrets.insert(field.name.clone(), secret);
                    }
                    Err(_) => {
                        return Ok(crate::adapter::types::ValidationResult {
                            valid: false,
                            identity: None,
                            message: Some(format!("missing secret: {}", field.name)),
                        });
                    }
                }
            }
        }
        let resolved = ResolvedProfile {
            app_id: app_id.to_string(),
            profile_name: profile_name.to_string(),
            secrets,
        };

        let result = adapter.validate(&ctx, &resolved)?;

        // Update auth state based on validation
        if result.valid {
            // Preserve existing identity from import if validate returns a
            // potentially wrong one (e.g. Firebase login:list shows all accounts)
            let identity_to_store = if result.identity.is_some() {
                // Only overwrite if profile doesn't already have an identity
                let existing = self.profile_manager.get_profile(app_id, profile_name).ok();
                let existing_identity = existing.as_ref().and_then(|p| p.auth_user.as_deref());
                match existing_identity {
                    Some(existing) if !existing.is_empty() => Some(existing.to_string()),
                    _ => result.identity.clone(),
                }
            } else {
                None
            };
            self.profile_manager.update_auth_state(
                app_id,
                profile_name,
                AuthState::Authenticated,
                identity_to_store.as_deref(),
            )?;
        } else {
            self.profile_manager.update_auth_state(
                app_id,
                profile_name,
                AuthState::Error,
                None,
            )?;
        }

        Ok(result)
    }

    /// Open a shell with the profile's env vars injected.
    pub fn shell(&self, app_id: &str, profile_name: Option<&str>) -> Result<ExitStatus> {
        let adapter = self.registry.get(app_id)?;

        let profile = match profile_name {
            Some(name) => self.profile_manager.get_profile(app_id, name)?,
            None => self.profile_manager.get_default_profile(app_id)?,
        };

        if profile.auth_state != AuthState::Authenticated {
            return Err(Error::NotAuthenticated {
                app_id: app_id.to_string(),
                profile_name: profile.profile_name.clone(),
            });
        }

        // Hydrate secrets
        let mut secrets = HashMap::new();
        for field in adapter.credential_fields() {
            if field.sensitive {
                let secret = self.profile_manager.retrieve_secret(&profile, &field.name)?;
                secrets.insert(field.name.clone(), secret);
            }
        }
        let resolved = ResolvedProfile {
            app_id: app_id.to_string(),
            profile_name: profile.profile_name.clone(),
            secrets,
        };

        let invocation = adapter.prepare_env(&resolved)?;

        // Launch user's shell with env injected
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let status = launcher::spawn(shell.as_ref(), &invocation, &[])?;

        Ok(status)
    }
}
