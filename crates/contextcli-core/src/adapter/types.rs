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

/// What auth pathways a provider supports.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthCapabilities {
    /// Has interactive login (login_args configured).
    pub interactive_login: bool,
    /// Accepts manual token paste (env_token or env_vars configured).
    pub manual_token: bool,
    /// Can import from a native config file.
    pub import_file: bool,
    /// Can import from macOS Keychain.
    pub import_keychain: bool,
    /// Can import via a token command (e.g., `gh auth token`).
    pub import_command: bool,
    /// Supports multi-account import (e.g., Firebase additionalAccounts).
    pub multi_account: bool,
    /// Supports config-dir isolation for concurrent profiles.
    pub config_dir_isolation: bool,
    /// Has a whoami/validation command.
    pub validate_whoami: bool,
}

impl AuthCapabilities {
    /// Number of distinct ways a user can authenticate.
    pub fn auth_path_count(&self) -> usize {
        [
            self.interactive_login,
            self.manual_token,
            self.import_file,
            self.import_keychain,
            self.import_command,
        ]
        .iter()
        .filter(|&&b| b)
        .count()
    }

    /// Short labels for the capabilities that are enabled.
    pub fn enabled_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.interactive_login {
            labels.push("login");
        }
        if self.manual_token {
            labels.push("manual");
        }
        if self.import_file {
            labels.push("file-import");
        }
        if self.import_keychain {
            labels.push("keychain");
        }
        if self.import_command {
            labels.push("token-cmd");
        }
        if self.multi_account {
            labels.push("multi-account");
        }
        if self.config_dir_isolation {
            labels.push("config-dir");
        }
        if self.validate_whoami {
            labels.push("whoami");
        }
        labels
    }
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
