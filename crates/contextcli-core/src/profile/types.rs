use serde::{Deserialize, Serialize};

/// Auth state for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Unauthenticated,
    Authenticated,
    Expired,
    Error,
}

impl AuthState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::Authenticated => "authenticated",
            Self::Expired => "expired",
            Self::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "authenticated" => Self::Authenticated,
            "expired" => Self::Expired,
            "error" => Self::Error,
            _ => Self::Unauthenticated,
        }
    }
}

/// Registered CLI app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub display_name: String,
    pub binary_path: Option<String>,
    pub adapter_version: String,
    pub support_level: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Auth profile for a CLI app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub app_id: String,
    pub profile_name: String,
    pub label: Option<String>,
    pub is_default: bool,
    pub auth_state: AuthState,
    pub auth_user: Option<String>,
    pub config_dir: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Reference to a secret stored in the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    pub id: String,
    pub profile_id: String,
    pub secret_key: String,
    pub vault_service: String,
    pub vault_account: String,
}

/// A project directory linked to an app profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLink {
    pub project_dir: String,
    pub app_id: String,
    pub profile_name: String,
}
