pub mod generic;
pub mod types;

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::path::Path;
use types::{
    AdapterContext, AuthCapabilities, AuthStrategy, CapturedCredentials, CredentialField,
    InvocationEnv, ResolvedProfile, ValidationResult,
};

/// Each supported CLI app implements this trait.
/// The adapter encodes how that specific CLI handles auth.
pub trait AppAdapter: Send + Sync {
    /// Unique id: "vercel", "gh", "wrangler".
    fn id(&self) -> &str;

    /// Display name: "Vercel", "GitHub CLI".
    fn display_name(&self) -> &str;

    /// Primary binary name: "vercel", "gh".
    fn binary_name(&self) -> &str;

    /// How this CLI handles auth.
    fn auth_strategy(&self) -> AuthStrategy;

    /// Perform login (interactive or token-based). Returns captured credentials.
    fn login(&self, ctx: &AdapterContext) -> Result<CapturedCredentials>;

    /// Validate an existing profile (e.g., whoami). Returns validity + identity.
    fn validate(&self, ctx: &AdapterContext, secrets: &ResolvedProfile) -> Result<ValidationResult>;

    /// Prepare the invocation environment for forwarding.
    /// Returns env vars to set + any extra CLI args to prepend.
    fn prepare_env(&self, profile: &ResolvedProfile) -> Result<InvocationEnv>;

    /// What credential fields this adapter stores per profile.
    fn credential_fields(&self) -> &[CredentialField];

    /// Adapter version string.
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// Support level: "tier1", "tier2", "tier3".
    fn support_level(&self) -> &str {
        "tier1"
    }

    /// What auth pathways this adapter supports.
    fn auth_capabilities(&self) -> AuthCapabilities {
        AuthCapabilities {
            interactive_login: false,
            manual_token: false,
            import_file: false,
            import_keychain: false,
            import_command: false,
            multi_account: false,
            config_dir_isolation: false,
            validate_whoami: false,
        }
    }

    /// Import credentials from the native CLI's existing config.
    /// Returns None if no existing auth is found.
    fn import_existing(&self) -> Result<Option<CapturedCredentials>> {
        Ok(None)
    }

    /// Import all accounts from the native CLI (multi-account tools like Firebase).
    /// Each entry is (profile_name_suggestion, credentials).
    /// Default: wraps import_existing into a single-element vec.
    fn import_all_accounts(&self) -> Result<Vec<(String, CapturedCredentials)>> {
        match self.import_existing()? {
            Some(creds) => Ok(vec![("default".to_string(), creds)]),
            None => Ok(vec![]),
        }
    }
}

/// Registry of all available app adapters.
pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn AppAdapter>>,
}

impl AdapterRegistry {
    /// Create registry from adapters.toml config.
    /// Writes default config if none exists.
    pub fn from_config(data_dir: &Path) -> Self {
        generic::write_default_config_if_missing(data_dir);
        let config = generic::load_adapters_config(data_dir);

        let mut r = Self {
            adapters: HashMap::new(),
        };

        for (id, def) in config.tool {
            let adapter = generic::GenericAdapter::from_def(id, def);
            r.register(Box::new(adapter));
        }

        r
    }

    pub fn register(&mut self, adapter: Box<dyn AppAdapter>) {
        self.adapters.insert(adapter.id().to_string(), adapter);
    }

    pub fn get(&self, id: &str) -> Result<&dyn AppAdapter> {
        self.adapters
            .get(id)
            .map(|a| a.as_ref())
            .ok_or_else(|| Error::AdapterNotFound(id.to_string()))
    }

    pub fn list(&self) -> Vec<&dyn AppAdapter> {
        self.adapters.values().map(|a| a.as_ref()).collect()
    }

    pub fn has(&self, id: &str) -> bool {
        self.adapters.contains_key(id)
    }
}
