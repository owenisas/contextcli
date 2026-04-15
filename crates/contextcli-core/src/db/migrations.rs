/// Embedded SQL migrations, applied sequentially.
/// Each entry: (version, description, sql)
pub static MIGRATIONS: &[(u32, &str, &str)] = &[
    (1, "initial schema", MIGRATION_001),
    (2, "project links", MIGRATION_002),
];

const MIGRATION_002: &str = r#"
CREATE TABLE project_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_dir TEXT NOT NULL,
    app_id TEXT NOT NULL,
    profile_name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_dir, app_id)
);
"#;

const MIGRATION_001: &str = r#"
CREATE TABLE apps (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    binary_path TEXT,
    adapter_version TEXT NOT NULL,
    support_level TEXT NOT NULL DEFAULT 'tier1',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    profile_name TEXT NOT NULL,
    label TEXT,
    is_default INTEGER NOT NULL DEFAULT 0,
    auth_state TEXT NOT NULL DEFAULT 'unauthenticated',
    auth_user TEXT,
    config_dir TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(app_id, profile_name)
);

CREATE TABLE secret_refs (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    secret_key TEXT NOT NULL,
    vault_service TEXT NOT NULL,
    vault_account TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(profile_id, secret_key)
);

CREATE TABLE activity_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    app_id TEXT,
    action TEXT NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX idx_one_default_per_app ON profiles(app_id) WHERE is_default = 1;
"#;
