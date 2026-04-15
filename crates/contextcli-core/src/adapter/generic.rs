//! Generic data-driven adapter.
//!
//! Reads tool definitions from `~/.contextcli/adapters.toml`.
//! Users add any CLI tool in a few lines — no Rust code, no recompilation.

use crate::adapter::types::{
    AdapterContext, AuthStrategy, CapturedCredentials, CredentialField, InvocationEnv,
    ResolvedProfile, ValidationResult,
};
use crate::adapter::AppAdapter;
use crate::error::{Error, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

// ── TOML config schema ──────────────────────────────────

/// Root of adapters.toml
#[derive(Debug, Deserialize)]
pub struct AdaptersConfig {
    #[serde(default)]
    pub tool: HashMap<String, ToolDef>,
}

/// One tool definition in adapters.toml
#[derive(Debug, Clone, Deserialize)]
pub struct ToolDef {
    /// Binary name to search PATH for
    pub binary: String,

    /// Human-readable name
    pub display_name: String,

    /// Single env var for token injection (most common pattern)
    #[serde(default)]
    pub env_token: Option<String>,

    /// Multiple env vars needed together (e.g., AWS)
    #[serde(default)]
    pub env_vars: Option<Vec<EnvVarDef>>,

    /// Config dir override env var (e.g., DOCKER_CONFIG, FLY_CONFIG_DIR)
    #[serde(default)]
    pub config_dir_env: Option<String>,

    /// Config dir override CLI flag (e.g., --config, --global-config)
    #[serde(default)]
    pub config_dir_flag: Option<String>,

    /// Login command args (e.g., ["login"] → runs `binary login`)
    #[serde(default)]
    pub login_args: Option<Vec<String>>,

    /// Whoami/validation command args (e.g., ["whoami"] → runs `binary whoami`)
    #[serde(default)]
    pub whoami_args: Option<Vec<String>>,

    /// Native config file path to read tokens from for import
    /// Supports ~ expansion and env vars
    #[serde(default)]
    pub native_config_path: Option<String>,

    /// JSON key path to extract token from native config (dot-separated)
    /// e.g., "token" or "user.accessToken"
    #[serde(default)]
    pub native_token_key: Option<String>,

    /// Keychain service name for native credential import
    /// e.g., "Supabase CLI"
    #[serde(default)]
    pub native_keychain_service: Option<String>,

    /// Keychain account name
    #[serde(default)]
    pub native_keychain_account: Option<String>,

    /// Whether the keychain value uses go-keyring base64 encoding
    #[serde(default)]
    pub native_keychain_go_base64: bool,

    /// Command to run to get the token (e.g., ["gh", "auth", "token"])
    #[serde(default)]
    pub native_token_command: Option<Vec<String>>,

    /// Command to get identity (e.g., ["gh", "auth", "status"])
    #[serde(default)]
    pub native_identity_command: Option<Vec<String>>,

    /// JSON key path to additional accounts array (e.g., "additionalAccounts")
    /// Each entry should have a token key and identity key
    #[serde(default)]
    pub native_additional_accounts_key: Option<String>,

    /// JSON key path to identity within each account (e.g., "user.email")
    #[serde(default)]
    pub native_account_identity_key: Option<String>,

    /// JSON key path to token within each account (e.g., "tokens.refresh_token")
    #[serde(default)]
    pub native_account_token_key: Option<String>,

    /// JSON key path to primary identity (e.g., "user.email")
    #[serde(default)]
    pub native_identity_key: Option<String>,

    /// Support level: tier1, tier2, tier3
    #[serde(default = "default_support_level")]
    pub support_level: String,
}

/// Named env var definition for multi-env tools
#[derive(Debug, Clone, Deserialize)]
pub struct EnvVarDef {
    /// Credential field name stored in vault (e.g., "access_key_id")
    pub field: String,

    /// Env var name to inject (e.g., "AWS_ACCESS_KEY_ID")
    pub env_var: String,

    /// Human-readable label
    #[serde(default)]
    pub display_name: Option<String>,

    /// Whether this field is required
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_support_level() -> String {
    "tier1".to_string()
}

fn default_true() -> bool {
    true
}

// ── Load config ─────────────────────────────────────────

/// Load adapters.toml from the data directory.
/// Returns empty config if file doesn't exist.
pub fn load_adapters_config(data_dir: &std::path::Path) -> AdaptersConfig {
    let config_path = data_dir.join("adapters.toml");
    match std::fs::read_to_string(&config_path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!("failed to parse {}: {e}", config_path.display());
                AdaptersConfig {
                    tool: HashMap::new(),
                }
            }
        },
        Err(_) => AdaptersConfig {
            tool: HashMap::new(),
        },
    }
}

/// Write the default adapters.toml if it doesn't exist yet.
pub fn write_default_config_if_missing(data_dir: &std::path::Path) {
    let config_path = data_dir.join("adapters.toml");
    if config_path.exists() {
        return;
    }
    if let Err(e) = std::fs::write(&config_path, DEFAULT_ADAPTERS_TOML) {
        tracing::warn!("failed to write default adapters.toml: {e}");
    }
}

// ── GenericAdapter ──────────────────────────────────────

/// A data-driven adapter constructed from a ToolDef.
pub struct GenericAdapter {
    id: String,
    def: ToolDef,
    credential_fields: Vec<CredentialField>,
}

impl GenericAdapter {
    pub fn from_def(id: String, def: ToolDef) -> Self {
        let credential_fields = Self::build_credential_fields(&def);
        Self {
            id,
            def,
            credential_fields,
        }
    }

    fn build_credential_fields(def: &ToolDef) -> Vec<CredentialField> {
        if let Some(env_vars) = &def.env_vars {
            // Multi-env: one field per env var
            env_vars
                .iter()
                .map(|v| CredentialField {
                    name: v.field.clone(),
                    display_name: v
                        .display_name
                        .clone()
                        .unwrap_or_else(|| v.field.clone()),
                    sensitive: true,
                    required: v.required,
                })
                .collect()
        } else {
            // Single token
            vec![CredentialField {
                name: "token".to_string(),
                display_name: "Token".to_string(),
                sensitive: true,
                required: true,
            }]
        }
    }

    /// Try to import from native config file (JSON)
    fn import_from_config_file(&self) -> Option<CapturedCredentials> {
        let path_str = self.def.native_config_path.as_ref()?;
        let path = expand_path(path_str);
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let key_path = self.def.native_token_key.as_deref().unwrap_or("token");
        let token = json_get_nested(&json, key_path)?.as_str()?;
        if token.is_empty() {
            return None;
        }

        let mut fields = HashMap::new();
        fields.insert("token".to_string(), SecretString::from(token.to_string()));

        // Try to get identity via whoami
        let identity = self.run_whoami_with_token(token);

        Some(CapturedCredentials { fields, identity })
    }

    /// Try to import from macOS Keychain (macOS only, no-op on other platforms)
    fn import_from_keychain(&self) -> Option<CapturedCredentials> {
        #[cfg(not(target_os = "macos"))]
        {
            return None;
        }

        #[cfg(target_os = "macos")]
        {
            let service = self.def.native_keychain_service.as_ref()?;
            let account = self.def.native_keychain_account.as_ref()?;

            let raw =
                security_framework::passwords::get_generic_password(service, account).ok()?;
            let raw_str = String::from_utf8(raw).ok()?;

            let token = if self.def.native_keychain_go_base64 {
                let b64 = raw_str.strip_prefix("go-keyring-base64:")?;
                base64_decode(b64)?
            } else {
                raw_str
            };

            if token.is_empty() {
                return None;
            }

            let mut fields = HashMap::new();
            fields.insert("token".to_string(), SecretString::from(token.clone()));

            let identity = self.run_whoami_with_token(&token);

            Some(CapturedCredentials { fields, identity })
        }
    }

    /// Try to import by running a command (e.g., `gh auth token`)
    fn import_from_command(&self) -> Option<CapturedCredentials> {
        let cmd_args = self.def.native_token_command.as_ref()?;
        if cmd_args.is_empty() {
            return None;
        }

        let output = Command::new(&cmd_args[0])
            .args(&cmd_args[1..])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if token.is_empty() {
            return None;
        }

        let mut fields = HashMap::new();
        fields.insert("token".to_string(), SecretString::from(token.clone()));

        // Get identity
        let identity = self.run_identity_command().or_else(|| self.run_whoami_with_token(&token));

        Some(CapturedCredentials { fields, identity })
    }

    /// Run identity command to extract username
    fn run_identity_command(&self) -> Option<String> {
        let cmd_args = self.def.native_identity_command.as_ref()?;
        if cmd_args.is_empty() {
            return None;
        }

        let output = Command::new(&cmd_args[0])
            .args(&cmd_args[1..])
            .output()
            .ok()?;

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        if combined.trim().is_empty() {
            return None;
        }

        Some(extract_identity(&combined))
    }

    /// Import all accounts from a multi-account config file (e.g., Firebase).
    fn import_multi_account_config(&self) -> Option<Vec<(String, CapturedCredentials)>> {
        let path_str = self.def.native_config_path.as_ref()?;
        let path = expand_path(path_str);
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let token_key = self.def.native_token_key.as_deref().unwrap_or("tokens.refresh_token");
        let identity_key = self.def.native_identity_key.as_deref().unwrap_or("user.email");
        let additional_key = self.def.native_additional_accounts_key.as_deref()?;
        let account_token_key = self.def.native_account_token_key.as_deref().unwrap_or(token_key);
        let account_identity_key = self.def.native_account_identity_key.as_deref().unwrap_or(identity_key);

        let mut accounts = Vec::new();

        // Primary account
        if let Some(token_val) = json_get_nested(&json, token_key) {
            if let Some(token) = token_val.as_str() {
                if !token.is_empty() {
                    let identity = json_get_nested(&json, identity_key)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let profile_name = identity
                        .as_ref()
                        .map(|e| e.split('@').next().unwrap_or("primary").to_string())
                        .unwrap_or_else(|| "primary".to_string());

                    let mut fields = HashMap::new();
                    fields.insert("token".to_string(), SecretString::from(token.to_string()));
                    accounts.push((
                        profile_name,
                        CapturedCredentials { fields, identity },
                    ));
                }
            }
        }

        // Additional accounts
        if let Some(additional) = json_get_nested(&json, additional_key) {
            if let Some(arr) = additional.as_array() {
                for entry in arr {
                    let token = json_get_nested(entry, account_token_key)
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if token.is_empty() {
                        continue;
                    }

                    let identity = json_get_nested(entry, account_identity_key)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let profile_name = identity
                        .as_ref()
                        .map(|e| e.split('@').next().unwrap_or("additional").to_string())
                        .unwrap_or_else(|| "additional".to_string());

                    let mut fields = HashMap::new();
                    fields.insert("token".to_string(), SecretString::from(token.to_string()));
                    accounts.push((
                        profile_name,
                        CapturedCredentials { fields, identity },
                    ));
                }
            }
        }

        if accounts.is_empty() {
            None
        } else {
            Some(accounts)
        }
    }

    /// Run whoami with token injected via env var.
    /// Extracts a clean identity (email or short username) from output.
    fn run_whoami_with_token(&self, token: &str) -> Option<String> {
        let whoami_args = self.def.whoami_args.as_ref()?;
        if whoami_args.is_empty() {
            return None;
        }

        let mut cmd = Command::new(&self.def.binary);
        for arg in whoami_args {
            cmd.arg(arg);
        }

        // Inject token via env var
        if let Some(env_var) = &self.def.env_token {
            cmd.env(env_var, token);
        }

        let output = cmd.output().ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return None;
        }

        // Extract clean identity from potentially verbose output
        Some(extract_identity(&stdout))
    }
}

/// Extract a clean identity from command output.
/// Tries to find an email, username, or falls back to first short line.
fn extract_identity(output: &str) -> String {
    // 1. Try to find an email address anywhere in the output
    for word in output.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.' && c != '-' && c != '_' && c != '+');
        if clean.contains('@') && clean.contains('.') && clean.len() > 3 {
            return clean.to_string();
        }
    }

    // 2. Try to find "account <name>" pattern (gh style)
    if let Some(pos) = output.find("account ") {
        let after = &output[pos + 8..];
        if let Some(name) = after.split_whitespace().next() {
            let clean = name.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
            if !clean.is_empty() {
                return clean.to_string();
            }
        }
    }

    // 3. If output is a single short line (< 60 chars), use it directly
    let first_line = output.lines().next().unwrap_or("").trim();
    if first_line.len() < 60 && !first_line.contains('|') && !first_line.contains("---") {
        return first_line.to_string();
    }

    // 4. Multi-line verbose output — just return first meaningful short line
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("---") || line.contains('|') || line.starts_with("WARNING") {
            continue;
        }
        if line.len() < 80 {
            return line.to_string();
        }
    }

    // 5. Fallback — truncate
    let truncated = &output[..output.len().min(40)];
    format!("{}...", truncated.trim())
}

impl AppAdapter for GenericAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.def.display_name
    }

    fn binary_name(&self) -> &str {
        &self.def.binary
    }

    fn auth_strategy(&self) -> AuthStrategy {
        if let Some(env_vars) = &self.def.env_vars {
            AuthStrategy::MultiEnv {
                env_vars: env_vars.iter().map(|v| v.env_var.clone()).collect(),
            }
        } else if let Some(env_var) = &self.def.env_token {
            if self.def.config_dir_env.is_some() || self.def.config_dir_flag.is_some() {
                AuthStrategy::EnvAndConfigDir {
                    env_var: env_var.clone(),
                    config_flag: self.def.config_dir_flag.clone(),
                    config_env: self.def.config_dir_env.clone(),
                }
            } else {
                AuthStrategy::EnvToken {
                    env_var: env_var.clone(),
                }
            }
        } else if self.def.config_dir_env.is_some() || self.def.config_dir_flag.is_some() {
            AuthStrategy::ConfigDir {
                override_flag: self.def.config_dir_flag.clone(),
                override_env: self.def.config_dir_env.clone(),
            }
        } else {
            AuthStrategy::EnvToken {
                env_var: format!("{}_TOKEN", self.id.to_uppercase()),
            }
        }
    }

    fn login(&self, ctx: &AdapterContext) -> Result<CapturedCredentials> {
        let default_login = vec!["login".to_string()];
        let login_args = self.def.login_args.as_deref().unwrap_or(&default_login);

        let mut cmd = Command::new(&self.def.binary);
        for arg in login_args {
            cmd.arg(arg);
        }

        // If config dir override is supported, use isolated dir
        if let Some(flag) = &self.def.config_dir_flag {
            cmd.arg(format!("{flag}={}", ctx.config_dir.display()));
        } else if let Some(env_var) = &self.def.config_dir_env {
            cmd.env(env_var, &ctx.config_dir);
        }

        cmd.stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        let status = cmd.status()?;
        if !status.success() {
            return Err(Error::LoginFailed(format!(
                "{} login exited with code {}",
                self.def.display_name,
                status.code().unwrap_or(-1)
            )));
        }

        // After login, try to capture credentials via import methods
        if let Some(creds) = self.import_from_command() {
            return Ok(creds);
        }
        if let Some(creds) = self.import_from_config_file() {
            return Ok(creds);
        }
        if let Some(creds) = self.import_from_keychain() {
            return Ok(creds);
        }

        // Login succeeded but couldn't capture token — user needs to provide manually
        Err(Error::LoginFailed(format!(
            "{} login succeeded but could not capture credentials. Use `contextcli import` with a token.",
            self.def.display_name
        )))
    }

    fn validate(
        &self,
        _ctx: &AdapterContext,
        secrets: &ResolvedProfile,
    ) -> Result<ValidationResult> {
        let whoami_args = match &self.def.whoami_args {
            Some(args) if !args.is_empty() => args,
            _ => {
                return Ok(ValidationResult {
                    valid: true,
                    identity: None,
                    message: Some("no whoami command configured".to_string()),
                });
            }
        };

        let mut cmd = Command::new(&self.def.binary);
        for arg in whoami_args {
            cmd.arg(arg);
        }

        // Inject credentials
        if let Some(env_vars) = &self.def.env_vars {
            for ev in env_vars {
                if let Ok(secret) = secrets.get_secret(&ev.field) {
                    cmd.env(&ev.env_var, secret.expose_secret());
                }
            }
        } else if let Some(env_var) = &self.def.env_token {
            if let Ok(token) = secrets.get_secret("token") {
                cmd.env(env_var, token.expose_secret());
            }
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            Ok(ValidationResult {
                valid: true,
                identity: if stdout.is_empty() {
                    None
                } else {
                    Some(extract_identity(&stdout))
                },
                message: None,
            })
        } else {
            Ok(ValidationResult {
                valid: false,
                identity: None,
                message: Some(if stderr.is_empty() { stdout } else { stderr }),
            })
        }
    }

    fn prepare_env(&self, profile: &ResolvedProfile) -> Result<InvocationEnv> {
        let mut env_vars = HashMap::new();

        if let Some(ev_defs) = &self.def.env_vars {
            for ev in ev_defs {
                if let Ok(secret) = profile.get_secret(&ev.field) {
                    env_vars.insert(ev.env_var.clone(), secret);
                }
            }
        } else if let Some(env_var) = &self.def.env_token {
            let token = profile.get_secret("token")?;
            env_vars.insert(env_var.clone(), token);
        }

        Ok(InvocationEnv {
            env_vars,
            extra_args: vec![],
            config_dir: None,
        })
    }

    fn credential_fields(&self) -> &[CredentialField] {
        &self.credential_fields
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn support_level(&self) -> &str {
        &self.def.support_level
    }

    fn import_existing(&self) -> Result<Option<CapturedCredentials>> {
        // Try each import method in priority order
        if let Some(creds) = self.import_from_command() {
            return Ok(Some(creds));
        }
        if let Some(creds) = self.import_from_keychain() {
            return Ok(Some(creds));
        }
        if let Some(creds) = self.import_from_config_file() {
            return Ok(Some(creds));
        }
        Ok(None)
    }

    fn import_all_accounts(&self) -> Result<Vec<(String, CapturedCredentials)>> {
        let mut accounts = Vec::new();

        // If this tool has multi-account config (like Firebase), parse all accounts
        if self.def.native_additional_accounts_key.is_some() {
            if let Some(all) = self.import_multi_account_config() {
                return Ok(all);
            }
        }

        // Fallback: single account via import_existing
        if let Some(creds) = self.import_existing()? {
            accounts.push(("default".to_string(), creds));
        }

        Ok(accounts)
    }
}

// ── Helpers ─────────────────────────────────────────────

fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

fn json_get_nested<'a>(value: &'a serde_json::Value, key_path: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in key_path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

fn base64_decode(input: &str) -> Option<String> {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &byte in input.as_bytes() {
        if byte == b'=' {
            break;
        }
        let val = match TABLE.iter().position(|&b| b == byte) {
            Some(v) => v as u32,
            None => continue,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    String::from_utf8(result).ok()
}

// ── Default adapters.toml ───────────────────────────────

pub const DEFAULT_ADAPTERS_TOML: &str = r#"# ContextCLI — Tool Adapter Definitions
# Add any CLI tool here. Four fields = full support.
#
# Required:
#   binary        = "tool-name"      # binary to find in PATH
#   display_name  = "Tool Name"      # shown in UI
#
# Auth (pick one pattern):
#   env_token     = "TOOL_TOKEN"     # single env var for token
#   env_vars      = [...]            # multiple env vars (see AWS example)
#
# Optional:
#   login_args           = ["login"]                    # args for login command
#   whoami_args          = ["whoami"]                    # args for validation
#   config_dir_env       = "TOOL_CONFIG_DIR"            # env var to override config dir
#   config_dir_flag      = "--config"                   # CLI flag for config dir
#   native_config_path   = "~/path/to/config.json"      # import from native config
#   native_token_key     = "token"                      # JSON key path in config
#   native_keychain_service = "Service Name"            # import from macOS Keychain
#   native_keychain_account = "account"
#   native_keychain_go_base64 = false                   # go-keyring encoding
#   native_token_command = ["tool", "auth", "token"]    # command that prints token
#   native_identity_command = ["tool", "auth", "status"] # command that prints identity
#   support_level = "tier1"

# ── Vercel ───────────────────────────────────────────────
[tool.vercel]
binary = "vercel"
display_name = "Vercel"
env_token = "VERCEL_TOKEN"
config_dir_flag = "--global-config"
login_args = ["login"]
whoami_args = ["whoami"]
native_config_path = "~/Library/Application Support/com.vercel.cli/auth.json"
native_token_key = "token"

# ── GitHub CLI ───────────────────────────────────────────
[tool.gh]
binary = "gh"
display_name = "GitHub CLI"
env_token = "GH_TOKEN"
config_dir_env = "GH_CONFIG_DIR"
login_args = ["auth", "login"]
whoami_args = ["auth", "status"]
native_token_command = ["gh", "auth", "token"]
native_identity_command = ["gh", "auth", "status"]

# ── Supabase ─────────────────────────────────────────────
[tool.supabase]
binary = "supabase"
display_name = "Supabase"
env_token = "SUPABASE_ACCESS_TOKEN"
login_args = ["login"]
# No whoami command — validation done by checking if token exists
native_keychain_service = "Supabase CLI"
native_keychain_account = "supabase"
native_keychain_go_base64 = true

# ── AWS CLI ──────────────────────────────────────────────
[tool.aws]
binary = "aws"
display_name = "AWS"
login_args = ["configure"]
whoami_args = ["sts", "get-caller-identity"]
support_level = "tier1"

[[tool.aws.env_vars]]
field = "access_key_id"
env_var = "AWS_ACCESS_KEY_ID"
display_name = "Access Key ID"

[[tool.aws.env_vars]]
field = "secret_access_key"
env_var = "AWS_SECRET_ACCESS_KEY"
display_name = "Secret Access Key"

[[tool.aws.env_vars]]
field = "session_token"
env_var = "AWS_SESSION_TOKEN"
display_name = "Session Token"
required = false

# ── Cloudflare Wrangler ──────────────────────────────────
[tool.wrangler]
binary = "wrangler"
display_name = "Cloudflare Wrangler"
env_token = "CLOUDFLARE_API_TOKEN"
login_args = ["login"]
whoami_args = ["whoami"]

# ── Netlify ──────────────────────────────────────────────
[tool.netlify]
binary = "netlify"
display_name = "Netlify"
env_token = "NETLIFY_AUTH_TOKEN"
login_args = ["login"]
whoami_args = ["status"]

# ── Fly.io ───────────────────────────────────────────────
[tool.fly]
binary = "fly"
display_name = "Fly.io"
env_token = "FLY_ACCESS_TOKEN"
config_dir_env = "FLY_CONFIG_DIR"
login_args = ["auth", "login"]
whoami_args = ["auth", "whoami"]
native_config_path = "~/.fly/config.yml"
native_token_key = "access_token"

# ── Railway ──────────────────────────────────────────────
[tool.railway]
binary = "railway"
display_name = "Railway"
env_token = "RAILWAY_TOKEN"
login_args = ["login"]
whoami_args = ["whoami"]
native_config_path = "~/.railway/config.json"
native_token_key = "user.accessToken"

# ── Heroku ───────────────────────────────────────────────
[tool.heroku]
binary = "heroku"
display_name = "Heroku"
env_token = "HEROKU_API_KEY"
login_args = ["login"]
whoami_args = ["auth:whoami"]

# ── DigitalOcean ─────────────────────────────────────────
[tool.doctl]
binary = "doctl"
display_name = "DigitalOcean"
env_token = "DIGITALOCEAN_ACCESS_TOKEN"
login_args = ["auth", "init"]
whoami_args = ["account", "get"]

# ── Terraform ────────────────────────────────────────────
[tool.terraform]
binary = "terraform"
display_name = "Terraform"
env_token = "TF_TOKEN_app_terraform_io"
login_args = ["login"]
support_level = "tier2"

# ── Docker ───────────────────────────────────────────────
[tool.docker]
binary = "docker"
display_name = "Docker"
config_dir_env = "DOCKER_CONFIG"
config_dir_flag = "--config"
login_args = ["login"]
support_level = "tier2"

# ── npm ──────────────────────────────────────────────────
[tool.npm]
binary = "npm"
display_name = "npm"
env_token = "NPM_TOKEN"
config_dir_env = "NPM_CONFIG_USERCONFIG"
login_args = ["login"]
whoami_args = ["whoami"]

# ── kubectl ──────────────────────────────────────────────
[tool.kubectl]
binary = "kubectl"
display_name = "Kubernetes"
config_dir_env = "KUBECONFIG"
config_dir_flag = "--kubeconfig"
whoami_args = ["auth", "whoami"]
support_level = "tier2"

# ── Firebase ─────────────────────────────────────────────
[tool.firebase]
binary = "firebase"
display_name = "Firebase"
env_token = "FIREBASE_TOKEN"
login_args = ["login"]
whoami_args = ["login:list"]
native_config_path = "~/.config/configstore/firebase-tools.json"
native_token_key = "tokens.refresh_token"
native_identity_key = "user.email"
native_additional_accounts_key = "additionalAccounts"
native_account_token_key = "tokens.refresh_token"
native_account_identity_key = "user.email"
support_level = "tier2"
"#;
