pub mod launcher;

use crate::adapter::types::{AdapterContext, ResolvedProfile};
use crate::adapter::{AdapterRegistry, AppAdapter};
use crate::error::{Error, Result};
use crate::profile::types::AuthState;
use crate::profile::ProfileManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::{SystemTime, UNIX_EPOCH};

/// Core router: resolve profile → prepare auth context → forward to native CLI.
pub struct Router<'a> {
    pub registry: &'a AdapterRegistry,
    pub profile_manager: &'a ProfileManager,
    pub data_dir: PathBuf,
}

impl<'a> Router<'a> {
    /// Forward a command to the native CLI under a specific profile.
    /// Checks .contextcli.toml in current directory for auto-profile and policies.
    ///
    /// Profile resolution order (no explicit `--profile`):
    ///   1. `.contextcli.toml` mapping
    ///   2. Explicit default profile
    ///   3. Sole profile for app (auto-promoted to default)
    ///   4. Auto-import from native CLI config (creates "default" profile)
    ///
    /// Token expiry handling:
    ///   - Expired → silent re-import from native CLI, then interactive re-login
    ///   - <24h remaining → warning printed to stderr
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

        // 3. Resolve profile: explicit flag > project config > default > sole > auto-import
        let resolved_profile_name = match profile_name {
            Some(name) => name.to_string(),
            None => {
                if let Some(ref pc) = project_config {
                    if let Some(mapped) = pc.profile_for(app_id) {
                        tracing::info!("auto-selected profile '{}' from .contextcli.toml", mapped);
                        mapped.to_string()
                    } else {
                        self.resolve_profile_name(app_id, adapter)?
                    }
                } else {
                    self.resolve_profile_name(app_id, adapter)?
                }
            }
        };

        let mut profile = self
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

        // 6. Check token expiry and refresh if needed
        self.check_and_refresh_token(app_id, &mut profile, adapter)?;

        // 7. Hydrate secrets from vault
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

        // 8. Ask adapter to prepare invocation env
        let invocation = adapter.prepare_env(&resolved)?;

        // 9. Find binary
        let binary_name = adapter.binary_name();
        let binary = which::which(binary_name)
            .map_err(|_| Error::BinaryNotFound(binary_name.to_string()))?;

        // 10. Launch
        let status = launcher::spawn(&binary, &invocation, forward_args)?;

        // 11. Log activity (no secrets)
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
    /// If `profile_name` is None, resolves via default → sole profile cascade.
    pub fn logout(&self, app_id: &str, profile_name: Option<&str>) -> Result<()> {
        let resolved_name = match profile_name {
            Some(name) => name.to_string(),
            None => {
                let adapter = self.registry.get(app_id)?;
                self.resolve_profile_name(app_id, adapter)?
            }
        };
        let profile_name = &resolved_name;
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
            // Record validation timestamp
            let _ = self.profile_manager.update_last_validated_at(app_id, profile_name);
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
    /// Uses same resolution + token refresh logic as `forward()`.
    /// Respects `.contextcli.toml` project config for profile mapping.
    pub fn shell(&self, app_id: &str, profile_name: Option<&str>) -> Result<ExitStatus> {
        let adapter = self.registry.get(app_id)?;

        // Check project config for auto-profile mapping (same as forward)
        let project_config = std::env::current_dir()
            .ok()
            .and_then(|cwd| crate::project::find_project_config(&cwd));

        let mut profile = match profile_name {
            Some(name) => self.profile_manager.get_profile(app_id, name)?,
            None => {
                let resolved = if let Some((ref pc, _)) = project_config {
                    if let Some(mapped) = pc.profile_for(app_id) {
                        tracing::info!("auto-selected profile '{}' from .contextcli.toml", mapped);
                        mapped.to_string()
                    } else {
                        self.resolve_profile_name(app_id, adapter)?
                    }
                } else {
                    self.resolve_profile_name(app_id, adapter)?
                };
                self.profile_manager.get_profile(app_id, &resolved)?
            }
        };

        if profile.auth_state != AuthState::Authenticated {
            return Err(Error::NotAuthenticated {
                app_id: app_id.to_string(),
                profile_name: profile.profile_name.clone(),
            });
        }

        // Check token expiry and refresh if needed
        self.check_and_refresh_token(app_id, &mut profile, adapter)?;

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

    // ── Private helpers ─────────────────────────────────────

    /// Resolve profile name when none specified explicitly.
    /// Cascade: default → sole profile (auto-promote) → auto-import from native CLI.
    fn resolve_profile_name(&self, app_id: &str, adapter: &dyn AppAdapter) -> Result<String> {
        // 1. Try explicit default
        if let Ok(p) = self.profile_manager.get_default_profile(app_id) {
            return Ok(p.profile_name);
        }

        // 2. Sole profile → auto-promote to default
        if let Some(p) = self.profile_manager.get_sole_profile(app_id) {
            tracing::info!(
                "auto-promoting sole profile '{}' to default for {}",
                p.profile_name,
                app_id
            );
            if let Err(e) = self.profile_manager.set_default(app_id, &p.profile_name) {
                tracing::warn!("failed to auto-promote profile '{}': {}", p.profile_name, e);
            }
            return Ok(p.profile_name);
        }

        // 3. Auto-import from native CLI (reuses self.import())
        tracing::info!(
            "no profile for {}, attempting auto-import from native CLI",
            app_id
        );
        let profile_name = "default";
        if self.import(app_id, profile_name)? {
            self.profile_manager.set_default(app_id, profile_name)?;
            let profile = self
                .profile_manager
                .get_profile(app_id, profile_name)?;
            let identity_msg = profile.auth_user.as_deref().unwrap_or("unknown");
            eprintln!(
                "✓ auto-imported {} credentials ({})",
                adapter.display_name(),
                identity_msg
            );
            return Ok(profile_name.to_string());
        }

        Err(Error::NoDefaultProfile(app_id.to_string()))
    }

    /// Check token expiry. If expired, try silent re-import then interactive re-login.
    /// If near expiry (<24h), warn to stderr but proceed.
    ///
    /// Sets `AuthState::Expired` before attempting refresh so the profile is in a
    /// safe state if the process crashes mid-refresh.
    fn check_and_refresh_token(
        &self,
        app_id: &str,
        profile: &mut crate::profile::types::Profile,
        adapter: &dyn AppAdapter,
    ) -> Result<()> {
        /// Small buffer to avoid race between check and subprocess using the token.
        const EXPIRY_BUFFER_SECS: i64 = 30;
        /// Warn when token expires within this window (24 hours).
        const NEAR_EXPIRY_SECS: i64 = 86400;

        let expires_at = match profile.token_expires_at {
            Some(ts) => ts,
            None => return Ok(()), // No expiry info — nothing to check
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let expires_in = expires_at - now;

        if expires_in <= EXPIRY_BUFFER_SECS {
            // Token expired (or about to) — mark Expired for crash safety
            eprintln!(
                "⚠ token expired for {}/{}",
                app_id, profile.profile_name
            );
            let _ = self.profile_manager.update_auth_state(
                app_id,
                &profile.profile_name,
                AuthState::Expired,
                profile.auth_user.as_deref(),
            );

            // Try silent re-import first
            if let Ok(Some(creds)) = adapter.import_existing() {
                for (key, value) in &creds.fields {
                    self.profile_manager.store_secret(profile, key, value)?;
                }
                self.profile_manager.update_auth_state(
                    app_id,
                    &profile.profile_name,
                    AuthState::Authenticated,
                    creds
                        .identity
                        .as_deref()
                        .or(profile.auth_user.as_deref()),
                )?;

                // Verify the re-imported token isn't also expired
                *profile = self
                    .profile_manager
                    .get_profile(app_id, &profile.profile_name)?;
                if let Some(new_exp) = profile.token_expires_at {
                    let new_now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    if new_exp <= new_now + EXPIRY_BUFFER_SECS {
                        eprintln!("  re-imported token is also expired, re-authenticating...");
                        self.login(app_id, &profile.profile_name)?;
                        eprintln!("✓ re-authenticated");
                        *profile = self
                            .profile_manager
                            .get_profile(app_id, &profile.profile_name)?;
                        return Ok(());
                    }
                }

                eprintln!("✓ refreshed credentials from native CLI config");
                return Ok(());
            }

            // Fall back to interactive re-login
            eprintln!("  re-authenticating...");
            self.login(app_id, &profile.profile_name)?;
            eprintln!("✓ re-authenticated");
            *profile = self
                .profile_manager
                .get_profile(app_id, &profile.profile_name)?;
        } else if expires_in < NEAR_EXPIRY_SECS {
            // Warn: expires within 24 hours
            let hours = expires_in / 3600;
            let mins = (expires_in % 3600) / 60;
            eprintln!(
                "⚠ token for {}/{} expires in {}h {}m",
                app_id, profile.profile_name, hours, mins
            );
        }

        Ok(())
    }
}
