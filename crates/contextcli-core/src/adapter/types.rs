use secrecy::SecretString;
use std::collections::HashMap;
use std::path::PathBuf;

/// How a CLI app handles authentication.
#[derive(Debug, Clone)]
pub enum AuthStrategy {
    /// Single env var token. E.g., VERCEL_TOKEN, NETLIFY_AUTH_TOKEN.
    EnvToken {
        env_var: String,
    },

    /// Isolated config directory with optional override flag/env.
    ConfigDir {
        override_flag: Option<String>,
        override_env: Option<String>,
    },

    /// Supports both env var and config dir (prefer env var for run).
    /// E.g., Vercel: VERCEL_TOKEN + --global-config
    EnvAndConfigDir {
        env_var: String,
        config_flag: Option<String>,
        config_env: Option<String>,
    },

    /// Multiple env vars needed together. E.g., AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY.
    MultiEnv {
        env_vars: Vec<String>,
    },
}

/// What prepare_env() returns — everything the router needs to launch the native CLI.
#[derive(Debug)]
pub struct InvocationEnv {
    /// Env vars to set on the child process.
    pub env_vars: HashMap<String, SecretString>,

    /// Extra CLI args to prepend before the user's forwarded args.
    pub extra_args: Vec<String>,

    /// Config dir override path (if using config dir strategy).
    pub config_dir: Option<PathBuf>,
}

/// Credentials captured after a login flow.
pub struct CapturedCredentials {
    /// field_name -> secret_value. E.g., {"token": "xxx"}.
    pub fields: HashMap<String, SecretString>,

    /// Identity string returned by validation (e.g., username/email).
    pub identity: Option<String>,
}

/// Result of validating an existing profile.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub identity: Option<String>,
    pub message: Option<String>,
}

/// Describes one credential field an adapter stores per profile.
#[derive(Debug, Clone)]
pub struct CredentialField {
    /// Internal key name. E.g., "token", "access_key_id".
    pub name: String,

    /// Human-readable label. E.g., "API Token".
    pub display_name: String,

    /// Whether this field is sensitive (stored in vault vs plain metadata).
    pub sensitive: bool,

    /// Whether login requires this field.
    pub required: bool,
}

/// Context passed to adapter methods during login/validate.
pub struct AdapterContext {
    /// Isolated config directory for this profile.
    pub config_dir: PathBuf,

    /// Profile name.
    pub profile_name: String,

    /// App ID.
    pub app_id: String,
}

/// A profile with its secrets resolved from the vault, ready for use.
pub struct ResolvedProfile {
    pub app_id: String,
    pub profile_name: String,
    pub secrets: HashMap<String, SecretString>,
}

impl ResolvedProfile {
    pub fn get_secret(&self, key: &str) -> crate::error::Result<SecretString> {
        self.secrets
            .get(key)
            .cloned()
            .ok_or_else(|| crate::error::Error::Vault(format!("secret not found: {key}")))
    }
}
