//! Database pool with split reader/writer connections in WAL mode.
//!
//! SQLite allows only one writer at a time. This module provides a `DatabasePool`
//! with a multi-connection reader pool for concurrent reads and a single-connection
//! writer pool for serialized writes. Both use WAL journal mode and enforce foreign keys.

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

/// Split read/write pool for SQLite with WAL mode.
///
/// - `reader`: Multi-connection pool (up to 8) for concurrent SELECT queries.
/// - `writer`: Single-connection pool for serialized INSERT/UPDATE/DELETE.
#[derive(Clone)]
pub struct DatabasePool {
    pub reader: SqlitePool,
    pub writer: SqlitePool,
}

impl DatabasePool {
    /// Create a new DatabasePool with split reader/writer connections.
    ///
    /// Runs migrations automatically on the writer pool.
    /// Both pools use WAL journal mode, foreign key enforcement, and 5-second busy timeout.
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let base_opts = SqliteConnectOptions::from_str(database_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5))
            .create_if_missing(true);

        let read_opts = base_opts.clone().read_only(true);
        let write_opts = base_opts;

        let writer = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(write_opts)
            .await?;

        // Run migrations on writer before opening reader pool
        sqlx::migrate!("../../migrations")
            .run(&writer)
            .await?;

        let reader = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(read_opts)
            .await?;

        Ok(Self { reader, writer })
    }
}

/// Returns the default database URL based on `BOTERNITY_DATA_DIR` env var,
/// falling back to `~/.boternity/boternity.db`.
pub fn default_database_url() -> String {
    let data_dir = std::env::var("BOTERNITY_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.boternity")
    });
    format!("sqlite://{data_dir}/boternity.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_creates_tables() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = DatabasePool::new(&url).await.unwrap();

        // Verify tables exist by querying sqlite_master
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(&pool.reader)
        .await
        .unwrap();

        let table_names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        assert!(table_names.contains(&"bots"), "bots table missing");
        assert!(table_names.contains(&"soul_versions"), "soul_versions table missing");
        assert!(table_names.contains(&"secrets"), "secrets table missing");
        assert!(table_names.contains(&"api_keys"), "api_keys table missing");
    }

    #[tokio::test]
    async fn test_pool_wal_mode() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_wal.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = DatabasePool::new(&url).await.unwrap();

        let result: (String,) =
            sqlx::query_as("PRAGMA journal_mode")
                .fetch_one(&pool.writer)
                .await
                .unwrap();

        assert_eq!(result.0.to_lowercase(), "wal");
    }

    #[tokio::test]
    async fn test_pool_foreign_keys_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_fk.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = DatabasePool::new(&url).await.unwrap();

        let result: (i32,) =
            sqlx::query_as("PRAGMA foreign_keys")
                .fetch_one(&pool.writer)
                .await
                .unwrap();

        assert_eq!(result.0, 1, "foreign keys should be enabled");
    }

    #[tokio::test]
    async fn test_default_database_url() {
        let url = default_database_url();
        assert!(url.starts_with("sqlite://"));
        assert!(url.ends_with("boternity.db"));
    }
}
