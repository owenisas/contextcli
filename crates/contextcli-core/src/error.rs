#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("credential vault error: {0}")]
    Vault(String),

    #[error("adapter not found: {0}")]
    AdapterNotFound(String),

    #[error("profile not found: {app_id}/{profile_name}")]
    ProfileNotFound {
        app_id: String,
        profile_name: String,
    },

    #[error("no default profile for app: {0}")]
    NoDefaultProfile(String),

    #[error("profile not authenticated: {app_id}/{profile_name}")]
    NotAuthenticated {
        app_id: String,
        profile_name: String,
    },

    #[error("binary not found: {0}")]
    BinaryNotFound(String),

    #[error("login failed: {0}")]
    LoginFailed(String),

    #[error("validation failed: {0}")]
    ValidationFailed(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

// For future Tauri IPC compatibility
impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
