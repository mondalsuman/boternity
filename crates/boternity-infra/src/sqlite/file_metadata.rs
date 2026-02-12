//! SQLite file metadata store implementation.
//!
//! Provides CRUD operations for bot file metadata and version tracking.
//! Actual file content lives on disk; this stores only metadata in SQLite.

use boternity_types::error::RepositoryError;
use boternity_types::storage::{FileVersion, StorageFile};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed file metadata store.
pub struct SqliteFileMetadataStore {
    pool: DatabasePool,
}

impl SqliteFileMetadataStore {
    /// Create a new file metadata store backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Save file metadata (insert or update).
    pub async fn save_file(&self, file: &StorageFile) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO bot_files (id, bot_id, filename, mime_type, size_bytes, version, is_indexed, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (bot_id, filename) DO UPDATE SET
                   size_bytes = excluded.size_bytes,
                   version = excluded.version,
                   is_indexed = excluded.is_indexed,
                   mime_type = excluded.mime_type,
                   updated_at = excluded.updated_at"#,
        )
        .bind(file.id.to_string())
        .bind(file.bot_id.to_string())
        .bind(&file.filename)
        .bind(&file.mime_type)
        .bind(file.size_bytes as i64)
        .bind(file.version as i64)
        .bind(if file.is_indexed { 1i64 } else { 0i64 })
        .bind(format_datetime(&file.created_at))
        .bind(format_datetime(&file.updated_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    /// Update file metadata fields (e.g., after re-indexing or new version).
    pub async fn update_file(&self, file: &StorageFile) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            r#"UPDATE bot_files
               SET size_bytes = ?, version = ?, is_indexed = ?, mime_type = ?, updated_at = ?
               WHERE id = ?"#,
        )
        .bind(file.size_bytes as i64)
        .bind(file.version as i64)
        .bind(if file.is_indexed { 1i64 } else { 0i64 })
        .bind(&file.mime_type)
        .bind(format_datetime(&file.updated_at))
        .bind(file.id.to_string())
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Get file metadata by bot ID and filename.
    pub async fn get_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> Result<Option<StorageFile>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM bot_files WHERE bot_id = ? AND filename = ?")
            .bind(bot_id.to_string())
            .bind(filename)
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let file_row = FileRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(file_row.into_storage_file()?))
            }
            None => Ok(None),
        }
    }

    /// List all files for a bot, ordered by filename.
    pub async fn list_files(
        &self,
        bot_id: &Uuid,
    ) -> Result<Vec<StorageFile>, RepositoryError> {
        let rows = sqlx::query("SELECT * FROM bot_files WHERE bot_id = ? ORDER BY filename")
            .bind(bot_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut files = Vec::with_capacity(rows.len());
        for row in &rows {
            let file_row = FileRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            files.push(file_row.into_storage_file()?);
        }

        Ok(files)
    }

    /// Delete a file and its versions (cascaded by FK).
    pub async fn delete_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM bot_files WHERE bot_id = ? AND filename = ?")
            .bind(bot_id.to_string())
            .bind(filename)
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Save a file version record.
    pub async fn save_version(&self, version: &FileVersion) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO bot_file_versions (id, file_id, version, size_bytes, created_at)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(version.id.to_string())
        .bind(version.file_id.to_string())
        .bind(version.version as i64)
        .bind(version.size_bytes as i64)
        .bind(format_datetime(&version.created_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    /// Get all versions of a file, ordered by version descending (latest first).
    pub async fn get_versions(
        &self,
        file_id: &Uuid,
    ) -> Result<Vec<FileVersion>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM bot_file_versions WHERE file_id = ? ORDER BY version DESC",
        )
        .bind(file_id.to_string())
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut versions = Vec::with_capacity(rows.len());
        for row in &rows {
            let version_row = VersionRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            versions.push(version_row.into_file_version()?);
        }

        Ok(versions)
    }
}

// ---------------------------------------------------------------------------
// Private Row types
// ---------------------------------------------------------------------------

struct FileRow {
    id: String,
    bot_id: String,
    filename: String,
    mime_type: String,
    size_bytes: i64,
    version: i64,
    is_indexed: i64,
    created_at: String,
    updated_at: String,
}

impl FileRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            bot_id: row.try_get("bot_id")?,
            filename: row.try_get("filename")?,
            mime_type: row.try_get("mime_type")?,
            size_bytes: row.try_get("size_bytes")?,
            version: row.try_get("version")?,
            is_indexed: row.try_get("is_indexed")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn into_storage_file(self) -> Result<StorageFile, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid file id: {e}")))?;
        let bot_id = Uuid::parse_str(&self.bot_id)
            .map_err(|e| RepositoryError::Query(format!("invalid bot_id: {e}")))?;
        let created_at = parse_datetime(&self.created_at)?;
        let updated_at = parse_datetime(&self.updated_at)?;

        Ok(StorageFile {
            id,
            bot_id,
            filename: self.filename,
            mime_type: self.mime_type,
            size_bytes: self.size_bytes as u64,
            version: self.version as u32,
            is_indexed: self.is_indexed != 0,
            created_at,
            updated_at,
        })
    }
}

struct VersionRow {
    id: String,
    file_id: String,
    version: i64,
    size_bytes: i64,
    created_at: String,
}

impl VersionRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            file_id: row.try_get("file_id")?,
            version: row.try_get("version")?,
            size_bytes: row.try_get("size_bytes")?,
            created_at: row.try_get("created_at")?,
        })
    }

    fn into_file_version(self) -> Result<FileVersion, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid version id: {e}")))?;
        let file_id = Uuid::parse_str(&self.file_id)
            .map_err(|e| RepositoryError::Query(format!("invalid file_id: {e}")))?;
        let created_at = parse_datetime(&self.created_at)?;

        Ok(FileVersion {
            id,
            file_id,
            version: self.version as u32,
            size_bytes: self.size_bytes as u64,
            created_at,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Query(format!("invalid datetime: {e}")))
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::pool::DatabasePool;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    async fn setup_bot(pool: &DatabasePool) -> Uuid {
        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind(format!("bot-{}", bot_id))
        .bind("Test Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        bot_id
    }

    fn make_file(bot_id: Uuid, filename: &str) -> StorageFile {
        let now = Utc::now();
        StorageFile {
            id: Uuid::now_v7(),
            bot_id,
            filename: filename.to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 1024,
            version: 1,
            is_indexed: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_save_and_get_file() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let file = make_file(bot_id, "notes.txt");
        store.save_file(&file).await.unwrap();

        let loaded = store.get_file(&bot_id, "notes.txt").await.unwrap().unwrap();
        assert_eq!(loaded.id, file.id);
        assert_eq!(loaded.filename, "notes.txt");
        assert_eq!(loaded.mime_type, "text/plain");
        assert_eq!(loaded.size_bytes, 1024);
        assert_eq!(loaded.version, 1);
        assert!(!loaded.is_indexed);
    }

    #[tokio::test]
    async fn test_get_nonexistent_file_returns_none() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let loaded = store.get_file(&bot_id, "missing.txt").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_update_file() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let mut file = make_file(bot_id, "data.csv");
        store.save_file(&file).await.unwrap();

        // Update: new version, mark as indexed
        file.version = 2;
        file.size_bytes = 2048;
        file.is_indexed = true;
        file.updated_at = Utc::now();
        store.update_file(&file).await.unwrap();

        let loaded = store.get_file(&bot_id, "data.csv").await.unwrap().unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.size_bytes, 2048);
        assert!(loaded.is_indexed);
    }

    #[tokio::test]
    async fn test_update_nonexistent_file_returns_not_found() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());

        let file = StorageFile {
            id: Uuid::now_v7(),
            bot_id: Uuid::now_v7(),
            filename: "phantom.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 0,
            version: 1,
            is_indexed: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = store.update_file(&file).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_files() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        store.save_file(&make_file(bot_id, "c.txt")).await.unwrap();
        store.save_file(&make_file(bot_id, "a.txt")).await.unwrap();
        store.save_file(&make_file(bot_id, "b.txt")).await.unwrap();

        let files = store.list_files(&bot_id).await.unwrap();
        assert_eq!(files.len(), 3);
        // Ordered by filename
        assert_eq!(files[0].filename, "a.txt");
        assert_eq!(files[1].filename, "b.txt");
        assert_eq!(files[2].filename, "c.txt");
    }

    #[tokio::test]
    async fn test_list_files_empty() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let files = store.list_files(&bot_id).await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_delete_file() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        store.save_file(&make_file(bot_id, "delete_me.txt")).await.unwrap();
        store.delete_file(&bot_id, "delete_me.txt").await.unwrap();

        let loaded = store.get_file(&bot_id, "delete_me.txt").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file_returns_not_found() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let result = store.delete_file(&bot_id, "nope.txt").await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn test_save_and_get_versions() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let file = make_file(bot_id, "versioned.txt");
        store.save_file(&file).await.unwrap();

        let v1 = FileVersion {
            id: Uuid::now_v7(),
            file_id: file.id,
            version: 1,
            size_bytes: 1024,
            created_at: Utc::now(),
        };
        let v2 = FileVersion {
            id: Uuid::now_v7(),
            file_id: file.id,
            version: 2,
            size_bytes: 2048,
            created_at: Utc::now(),
        };

        store.save_version(&v1).await.unwrap();
        store.save_version(&v2).await.unwrap();

        let versions = store.get_versions(&file.id).await.unwrap();
        assert_eq!(versions.len(), 2);
        // Ordered by version DESC (latest first)
        assert_eq!(versions[0].version, 2);
        assert_eq!(versions[0].size_bytes, 2048);
        assert_eq!(versions[1].version, 1);
        assert_eq!(versions[1].size_bytes, 1024);
    }

    #[tokio::test]
    async fn test_delete_file_cascades_versions() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let file = make_file(bot_id, "cascade.txt");
        store.save_file(&file).await.unwrap();

        let v1 = FileVersion {
            id: Uuid::now_v7(),
            file_id: file.id,
            version: 1,
            size_bytes: 512,
            created_at: Utc::now(),
        };
        store.save_version(&v1).await.unwrap();

        store.delete_file(&bot_id, "cascade.txt").await.unwrap();

        // Versions should be cascade-deleted
        let versions = store.get_versions(&file.id).await.unwrap();
        assert!(versions.is_empty());
    }

    #[tokio::test]
    async fn test_bot_isolation() {
        let pool = test_pool().await;
        let store = SqliteFileMetadataStore::new(pool.clone());
        let bot_a = setup_bot(&pool).await;
        let bot_b = setup_bot(&pool).await;

        store.save_file(&make_file(bot_a, "shared_name.txt")).await.unwrap();
        store.save_file(&make_file(bot_b, "shared_name.txt")).await.unwrap();

        let a_files = store.list_files(&bot_a).await.unwrap();
        let b_files = store.list_files(&bot_b).await.unwrap();
        assert_eq!(a_files.len(), 1);
        assert_eq!(b_files.len(), 1);
        // Different IDs
        assert_ne!(a_files[0].id, b_files[0].id);
    }
}
