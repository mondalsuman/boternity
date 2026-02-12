//! Local filesystem file store implementation.
//!
//! Implements the `FileStore` trait from `boternity-core` with files stored at
//! `{base_dir}/bots/{slug}/files/`. Version history is preserved in a
//! `.versions/` subdirectory alongside each file.
//!
//! File metadata is tracked in SQLite via `SqliteFileMetadataStore` (03-05);
//! this module handles the actual bytes on disk plus the `FileStore` trait glue.

use std::path::{Path, PathBuf};

use boternity_core::storage::file_store::FileStore;
use boternity_types::error::RepositoryError;
use boternity_types::storage::{FileVersion, StorageFile, MAX_FILE_SIZE_BYTES};
use chrono::Utc;
use uuid::Uuid;

use crate::sqlite::file_metadata::SqliteFileMetadataStore;

/// Local filesystem-backed file store with version history.
///
/// Directory layout per bot:
/// ```text
/// {base_dir}/bots/{slug}/files/
///   notes.txt
///   report.md
///   .versions/
///     notes.txt.v1
///     notes.txt.v2
///     report.md.v1
/// ```
///
/// On each save:
/// 1. Copy current file to `.versions/{filename}.v{version}` (if exists)
/// 2. Write new content to `{filename}`
/// 3. Record metadata in SQLite
/// 4. Record version in SQLite
pub struct LocalFileStore {
    base_dir: PathBuf,
    metadata_store: SqliteFileMetadataStore,
}

impl LocalFileStore {
    /// Create a new file store rooted at `base_dir`.
    ///
    /// Files for bot `slug` will be stored under `{base_dir}/bots/{slug}/files/`.
    pub fn new(base_dir: PathBuf, metadata_store: SqliteFileMetadataStore) -> Self {
        Self {
            base_dir,
            metadata_store,
        }
    }

    /// Compute the files directory for a given bot slug.
    fn bot_files_dir(&self, slug: &str) -> PathBuf {
        self.base_dir.join("bots").join(slug).join("files")
    }

    /// Compute the versions directory for a given bot slug.
    fn bot_versions_dir(&self, slug: &str) -> PathBuf {
        self.bot_files_dir(slug).join(".versions")
    }

    /// Compute the file path on disk.
    fn file_path(&self, slug: &str, filename: &str) -> PathBuf {
        self.bot_files_dir(slug).join(filename)
    }

    /// Compute the versioned file path.
    fn version_path(&self, slug: &str, filename: &str, version: u32) -> PathBuf {
        self.bot_versions_dir(slug)
            .join(format!("{}.v{}", filename, version))
    }

    /// Detect MIME type from file extension.
    fn detect_mime(filename: &str) -> String {
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            // Text
            "txt" => "text/plain",
            "md" | "markdown" => "text/markdown",
            "csv" => "text/csv",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "xml" => "text/xml",
            "yaml" | "yml" => "text/yaml",
            "toml" => "text/toml",

            // Code
            "rs" => "text/x-rust",
            "py" => "text/x-python",
            "js" => "text/javascript",
            "ts" => "text/typescript",
            "json" => "application/json",
            "sh" | "bash" => "text/x-shellscript",
            "sql" => "text/x-sql",
            "go" => "text/x-go",
            "java" => "text/x-java",
            "c" | "h" => "text/x-c",
            "cpp" | "hpp" | "cc" | "cxx" => "text/x-c++",

            // Documents
            "pdf" => "application/pdf",
            "doc" | "docx" => "application/msword",

            // Images
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",

            // Archives
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",

            // Default
            _ => "application/octet-stream",
        }
        .to_string()
    }

    /// Check whether a MIME type represents indexable text content.
    pub fn is_text_mime(mime: &str) -> bool {
        mime.starts_with("text/") || mime == "application/json"
    }

}

impl FileStore for LocalFileStore {
    async fn save_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
        data: &[u8],
    ) -> Result<StorageFile, RepositoryError> {
        // Enforce 50MB size limit
        if data.len() as u64 > MAX_FILE_SIZE_BYTES {
            return Err(RepositoryError::Conflict(format!(
                "File exceeds maximum size of {} bytes (got {} bytes)",
                MAX_FILE_SIZE_BYTES,
                data.len()
            )));
        }

        // Validate filename (no path traversal)
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(RepositoryError::Conflict(
                "Filename must not contain path separators or '..'".to_string(),
            ));
        }

        let mime_type = Self::detect_mime(filename);
        let now = Utc::now();

        // Check if file already exists (update vs create)
        let existing = self.metadata_store.get_file(bot_id, filename).await?;

        let (file, new_version) = if let Some(mut existing_file) = existing {
            // Existing file: bump version
            let new_version = existing_file.version + 1;
            existing_file.version = new_version;
            existing_file.size_bytes = data.len() as u64;
            existing_file.mime_type = mime_type;
            existing_file.updated_at = now;

            (existing_file, new_version)
        } else {
            // New file
            let file = StorageFile {
                id: Uuid::now_v7(),
                bot_id: *bot_id,
                filename: filename.to_string(),
                mime_type,
                size_bytes: data.len() as u64,
                version: 1,
                is_indexed: false,
                created_at: now,
                updated_at: now,
            };
            (file, 1)
        };

        // Use bot_id as directory name (simple form) for filesystem storage
        let slug = bot_id.simple().to_string();
        let files_dir = self.bot_files_dir(&slug);
        let versions_dir = self.bot_versions_dir(&slug);

        // Ensure directories exist
        tokio::fs::create_dir_all(&files_dir)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to create files dir: {e}")))?;
        tokio::fs::create_dir_all(&versions_dir)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to create versions dir: {e}")))?;

        let file_path = self.file_path(&slug, filename);

        // If file exists on disk, copy current content to version archive
        if file_path.exists() {
            let prev_version = new_version - 1;
            let version_path = self.version_path(&slug, filename, prev_version);
            tokio::fs::copy(&file_path, &version_path).await.map_err(|e| {
                RepositoryError::Query(format!("Failed to archive version {prev_version}: {e}"))
            })?;
        }

        // Write new content
        tokio::fs::write(&file_path, data)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to write file: {e}")))?;

        // Save metadata to SQLite
        self.metadata_store.save_file(&file).await?;

        // Record version
        let version_record = FileVersion {
            id: Uuid::now_v7(),
            file_id: file.id,
            version: new_version,
            size_bytes: data.len() as u64,
            created_at: now,
        };
        self.metadata_store.save_version(&version_record).await?;

        Ok(file)
    }

    async fn get_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> Result<Vec<u8>, RepositoryError> {
        // Verify file exists in metadata
        let _file = self
            .metadata_store
            .get_file(bot_id, filename)
            .await?
            .ok_or(RepositoryError::NotFound)?;

        let slug = bot_id.simple().to_string();
        let file_path = self.file_path(&slug, filename);

        tokio::fs::read(&file_path)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to read file: {e}")))
    }

    async fn delete_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> Result<(), RepositoryError> {
        // Get file metadata (also verifies existence)
        let file = self
            .metadata_store
            .get_file(bot_id, filename)
            .await?
            .ok_or(RepositoryError::NotFound)?;

        let slug = bot_id.simple().to_string();
        let file_path = self.file_path(&slug, filename);

        // Delete the main file from disk
        if file_path.exists() {
            tokio::fs::remove_file(&file_path).await.map_err(|e| {
                RepositoryError::Query(format!("Failed to delete file: {e}"))
            })?;
        }

        // Delete all version files from disk
        for v in 1..=file.version {
            let version_path = self.version_path(&slug, filename, v);
            if version_path.exists() {
                let _ = tokio::fs::remove_file(&version_path).await;
            }
        }

        // Delete metadata (cascades to versions via FK)
        self.metadata_store.delete_file(bot_id, filename).await?;

        Ok(())
    }

    async fn list_files(
        &self,
        bot_id: &Uuid,
    ) -> Result<Vec<StorageFile>, RepositoryError> {
        self.metadata_store.list_files(bot_id).await
    }

    async fn get_file_info(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> Result<StorageFile, RepositoryError> {
        self.metadata_store
            .get_file(bot_id, filename)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn get_versions(
        &self,
        file_id: &Uuid,
    ) -> Result<Vec<FileVersion>, RepositoryError> {
        self.metadata_store.get_versions(file_id).await
    }
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

    async fn make_store(pool: DatabasePool) -> (LocalFileStore, tempfile::TempDir) {
        let base_dir = tempfile::tempdir().unwrap();
        let metadata_store = SqliteFileMetadataStore::new(pool);
        let store = LocalFileStore::new(base_dir.path().to_path_buf(), metadata_store);
        (store, base_dir)
    }

    #[tokio::test]
    async fn test_save_and_get_roundtrip() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        let data = b"Hello, world!";
        let file = store.save_file(&bot_id, "hello.txt", data).await.unwrap();

        assert_eq!(file.filename, "hello.txt");
        assert_eq!(file.mime_type, "text/plain");
        assert_eq!(file.size_bytes, 13);
        assert_eq!(file.version, 1);
        assert!(!file.is_indexed);

        let content = store.get_file(&bot_id, "hello.txt").await.unwrap();
        assert_eq!(content, data);
    }

    #[tokio::test]
    async fn test_save_creates_version_history() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        // Save v1
        let file = store
            .save_file(&bot_id, "doc.md", b"# Version 1")
            .await
            .unwrap();
        assert_eq!(file.version, 1);

        // Save v2
        let file = store
            .save_file(&bot_id, "doc.md", b"# Version 2")
            .await
            .unwrap();
        assert_eq!(file.version, 2);

        // Current content should be v2
        let content = store.get_file(&bot_id, "doc.md").await.unwrap();
        assert_eq!(content, b"# Version 2");

        // Version history should have 2 entries
        let versions = store.get_versions(&file.id).await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, 2); // latest first
        assert_eq!(versions[1].version, 1);

        // Version file should exist on disk
        let slug = bot_id.simple().to_string();
        let v1_path = store.version_path(&slug, "doc.md", 1);
        let v1_content = tokio::fs::read(&v1_path).await.unwrap();
        assert_eq!(v1_content, b"# Version 1");
    }

    #[tokio::test]
    async fn test_delete_removes_files_and_versions() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        store
            .save_file(&bot_id, "temp.txt", b"temporary")
            .await
            .unwrap();
        store
            .save_file(&bot_id, "temp.txt", b"temporary v2")
            .await
            .unwrap();

        store.delete_file(&bot_id, "temp.txt").await.unwrap();

        // Get should fail
        let result = store.get_file(&bot_id, "temp.txt").await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));

        // Disk files should be gone
        let slug = bot_id.simple().to_string();
        let file_path = store.file_path(&slug, "temp.txt");
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_list_files() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        store
            .save_file(&bot_id, "c.txt", b"c content")
            .await
            .unwrap();
        store
            .save_file(&bot_id, "a.txt", b"a content")
            .await
            .unwrap();
        store
            .save_file(&bot_id, "b.txt", b"b content")
            .await
            .unwrap();

        let files = store.list_files(&bot_id).await.unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].filename, "a.txt");
        assert_eq!(files[1].filename, "b.txt");
        assert_eq!(files[2].filename, "c.txt");
    }

    #[tokio::test]
    async fn test_get_file_info() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        store
            .save_file(&bot_id, "info.txt", b"information")
            .await
            .unwrap();

        let info = store.get_file_info(&bot_id, "info.txt").await.unwrap();
        assert_eq!(info.filename, "info.txt");
        assert_eq!(info.size_bytes, 11);
    }

    #[tokio::test]
    async fn test_get_nonexistent_file_returns_not_found() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        let result = store.get_file(&bot_id, "nonexistent.txt").await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file_returns_not_found() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        let result = store.delete_file(&bot_id, "nonexistent.txt").await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn test_file_size_limit() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        // Create data exceeding 50MB
        let data = vec![0u8; (MAX_FILE_SIZE_BYTES + 1) as usize];
        let result = store.save_file(&bot_id, "huge.bin", &data).await;
        assert!(matches!(result, Err(RepositoryError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_path_traversal_rejected() {
        let pool = test_pool().await;
        let bot_id = setup_bot(&pool).await;
        let (store, _dir) = make_store(pool).await;

        let result = store
            .save_file(&bot_id, "../../../etc/passwd", b"evil")
            .await;
        assert!(matches!(result, Err(RepositoryError::Conflict(_))));

        let result = store
            .save_file(&bot_id, "sub/dir/file.txt", b"evil")
            .await;
        assert!(matches!(result, Err(RepositoryError::Conflict(_))));
    }

    #[test]
    fn test_detect_mime() {
        assert_eq!(LocalFileStore::detect_mime("file.txt"), "text/plain");
        assert_eq!(LocalFileStore::detect_mime("doc.md"), "text/markdown");
        assert_eq!(LocalFileStore::detect_mime("data.json"), "application/json");
        assert_eq!(LocalFileStore::detect_mime("image.png"), "image/png");
        assert_eq!(LocalFileStore::detect_mime("code.rs"), "text/x-rust");
        assert_eq!(
            LocalFileStore::detect_mime("unknown.xyz"),
            "application/octet-stream"
        );
        assert_eq!(
            LocalFileStore::detect_mime("no_extension"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_is_text_mime() {
        assert!(LocalFileStore::is_text_mime("text/plain"));
        assert!(LocalFileStore::is_text_mime("text/markdown"));
        assert!(LocalFileStore::is_text_mime("text/x-rust"));
        assert!(LocalFileStore::is_text_mime("application/json"));
        assert!(!LocalFileStore::is_text_mime("image/png"));
        assert!(!LocalFileStore::is_text_mime("application/pdf"));
        assert!(!LocalFileStore::is_text_mime("application/octet-stream"));
    }
}
