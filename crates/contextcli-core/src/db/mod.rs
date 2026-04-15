pub mod migrations;

use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;

/// Open (or create) the SQLite database and run pending migrations.
pub fn open_and_migrate(db_path: &Path) -> Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;

    // Enable WAL mode and foreign keys
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )?;

    run_migrations(&conn)?;
    Ok(conn)
}

/// Open an in-memory database (for tests).
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    // Create schema_version table if missing
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    let current_version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for &(version, description, sql) in migrations::MIGRATIONS {
        if version > current_version {
            tracing::info!(version, description, "applying migration");
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_version (version, description) VALUES (?1, ?2)",
                rusqlite::params![version, description],
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_apply_cleanly() {
        let conn = open_in_memory().expect("should open in-memory db");
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 2);
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = open_in_memory().expect("first open");
        run_migrations(&conn).expect("second migration run should be idempotent");
    }

    #[test]
    fn test_tables_exist() {
        let conn = open_in_memory().unwrap();
        // Verify all tables created
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"apps".to_string()));
        assert!(tables.contains(&"profiles".to_string()));
        assert!(tables.contains(&"secret_refs".to_string()));
        assert!(tables.contains(&"activity_log".to_string()));
    }
}
