//! LanceDB vector store wrapper for connection management and table operations.
//!
//! Provides `LanceVectorStore` which wraps a `lancedb::Connection` and offers
//! helper methods for table lifecycle (create, open, drop) using Arrow schemas.
//!
//! This is the infrastructure layer only. Trait implementations for
//! `VectorMemoryStore` and `SharedMemoryStore` live in Plans 03-07 and 03-09.

use std::path::PathBuf;
use std::sync::Arc;

use arrow_schema::Schema;
use uuid::Uuid;

/// LanceDB vector store wrapper for connection and table management.
///
/// Manages a single LanceDB connection at a filesystem path.
/// Each bot gets its own memory table (`bot_memory_{bot_id}`) and
/// file chunks table (`file_chunks_{bot_id}`).
/// A single `shared_memory` table is shared across all bots.
pub struct LanceVectorStore {
    db: lancedb::Connection,
    base_path: PathBuf,
}

impl LanceVectorStore {
    /// Open or create a LanceDB vector store at the given path.
    ///
    /// Creates the directory if it does not exist.
    /// Default path: `~/.boternity/vector_store`
    pub async fn new(base_path: PathBuf) -> Result<Self, lancedb::Error> {
        // Ensure directory exists
        std::fs::create_dir_all(&base_path).map_err(|e| lancedb::Error::CreateDir {
            path: base_path.display().to_string(),
            source: e,
        })?;

        let uri = base_path
            .to_str()
            .ok_or_else(|| lancedb::Error::InvalidInput {
                message: format!(
                    "Path contains invalid UTF-8: {}",
                    base_path.display()
                ),
            })?;

        let db = lancedb::connect(uri).execute().await?;

        Ok(Self { db, base_path })
    }

    /// Open or create a LanceDB vector store at the default path.
    ///
    /// Default: `~/.boternity/vector_store`
    pub async fn default() -> Result<Self, lancedb::Error> {
        let base_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".boternity")
            .join("vector_store");

        Self::new(base_path).await
    }

    /// Ensure a table exists with the given schema.
    ///
    /// If the table already exists, opens it. If not, creates an empty table
    /// with the provided schema.
    pub async fn ensure_table(
        &self,
        table_name: &str,
        schema: Arc<Schema>,
    ) -> Result<lancedb::Table, lancedb::Error> {
        // Try to open the existing table first
        match self.db.open_table(table_name).execute().await {
            Ok(table) => Ok(table),
            Err(lancedb::Error::TableNotFound { .. }) => {
                // Table doesn't exist, create it empty
                self.db
                    .create_empty_table(table_name, schema)
                    .execute()
                    .await
            }
            Err(e) => Err(e),
        }
    }

    /// Check if a table exists in the database.
    pub async fn table_exists(&self, table_name: &str) -> bool {
        self.db
            .open_table(table_name)
            .execute()
            .await
            .is_ok()
    }

    /// Drop a table from the database.
    ///
    /// Returns Ok(()) even if the table does not exist (idempotent).
    pub async fn drop_table(&self, table_name: &str) -> Result<(), lancedb::Error> {
        match self.db.drop_table(table_name, &[]).await {
            Ok(()) => Ok(()),
            Err(lancedb::Error::TableNotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// List all table names in the database.
    pub async fn table_names(&self) -> Result<Vec<String>, lancedb::Error> {
        self.db.table_names().execute().await
    }

    /// Get a reference to the underlying LanceDB connection.
    pub fn connection(&self) -> &lancedb::Connection {
        &self.db
    }

    /// Get the base path of the vector store.
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Generate the table name for a bot's personal memory table.
    pub fn bot_table_name(bot_id: &Uuid) -> String {
        format!("bot_memory_{}", bot_id.simple())
    }

    /// Get the table name for the shared memory table.
    pub fn shared_table_name() -> &'static str {
        "shared_memory"
    }

    /// Generate the table name for a bot's file chunks table.
    pub fn file_chunks_table_name(bot_id: &Uuid) -> String {
        format!("file_chunks_{}", bot_id.simple())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::schema::{bot_memory_schema, file_chunks_schema, shared_memory_schema};

    #[tokio::test]
    async fn test_connection_opens_successfully() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create vector store");

        // Connection should be alive -- listing tables should work
        let tables = store.table_names().await.expect("Failed to list tables");
        assert!(tables.is_empty());
    }

    #[tokio::test]
    async fn test_ensure_table_creates_and_reopens() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create vector store");

        let schema = Arc::new(bot_memory_schema());

        // First call: creates the table
        let table = store
            .ensure_table("test_bot_memory", schema.clone())
            .await
            .expect("Failed to create table");

        let count = table
            .count_rows(None)
            .await
            .expect("Failed to count rows");
        assert_eq!(count, 0);

        // Second call: opens the existing table
        let _table2 = store
            .ensure_table("test_bot_memory", schema)
            .await
            .expect("Failed to reopen table");
    }

    #[tokio::test]
    async fn test_shared_memory_table_creation() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create vector store");

        let schema = Arc::new(shared_memory_schema());
        let _table = store
            .ensure_table(LanceVectorStore::shared_table_name(), schema)
            .await
            .expect("Failed to create shared memory table");

        assert!(store.table_exists(LanceVectorStore::shared_table_name()).await);
    }

    #[tokio::test]
    async fn test_file_chunks_table_creation() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create vector store");

        let bot_id = Uuid::new_v4();
        let table_name = LanceVectorStore::file_chunks_table_name(&bot_id);
        let schema = Arc::new(file_chunks_schema());

        let _table = store
            .ensure_table(&table_name, schema)
            .await
            .expect("Failed to create file chunks table");

        assert!(store.table_exists(&table_name).await);
    }

    #[tokio::test]
    async fn test_drop_table_idempotent() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create vector store");

        let schema = Arc::new(bot_memory_schema());
        store
            .ensure_table("to_drop", schema)
            .await
            .expect("Failed to create table");

        assert!(store.table_exists("to_drop").await);

        // First drop should succeed
        store
            .drop_table("to_drop")
            .await
            .expect("Failed to drop table");

        assert!(!store.table_exists("to_drop").await);

        // Second drop should also succeed (idempotent)
        store
            .drop_table("to_drop")
            .await
            .expect("Second drop should be idempotent");
    }

    #[test]
    fn test_table_name_generation() {
        let bot_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        assert_eq!(
            LanceVectorStore::bot_table_name(&bot_id),
            "bot_memory_550e8400e29b41d4a716446655440000"
        );
        assert_eq!(
            LanceVectorStore::file_chunks_table_name(&bot_id),
            "file_chunks_550e8400e29b41d4a716446655440000"
        );
        assert_eq!(LanceVectorStore::shared_table_name(), "shared_memory");
    }

    #[tokio::test]
    async fn test_table_names_lists_all_tables() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create vector store");

        let schema = Arc::new(bot_memory_schema());
        store
            .ensure_table("table_a", schema.clone())
            .await
            .expect("Failed to create table_a");
        store
            .ensure_table("table_b", schema)
            .await
            .expect("Failed to create table_b");

        let mut names = store.table_names().await.expect("Failed to list tables");
        names.sort();
        assert_eq!(names, vec!["table_a", "table_b"]);
    }
}
