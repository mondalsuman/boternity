//! File store trait.
//!
//! Defines the interface for bot file storage with versioning.
//! Implementations live in boternity-infra.

use boternity_types::error::RepositoryError;
use boternity_types::storage::{FileVersion, StorageFile};
use uuid::Uuid;

/// Trait for bot file storage with versioning.
///
/// Uses RPITIT (native async fn in traits, Rust 2024 edition).
/// Implementations live in boternity-infra.
pub trait FileStore: Send + Sync {
    /// Save a file (creates new or adds a new version).
    fn save_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
        data: &[u8],
    ) -> impl std::future::Future<Output = Result<StorageFile, RepositoryError>> + Send;

    /// Retrieve the latest version of a file's content.
    fn get_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, RepositoryError>> + Send;

    /// Delete a file and all its versions.
    fn delete_file(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// List all files for a bot.
    fn list_files(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<StorageFile>, RepositoryError>> + Send;

    /// Get metadata for a specific file.
    fn get_file_info(
        &self,
        bot_id: &Uuid,
        filename: &str,
    ) -> impl std::future::Future<Output = Result<StorageFile, RepositoryError>> + Send;

    /// Get all versions of a file.
    fn get_versions(
        &self,
        file_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<FileVersion>, RepositoryError>> + Send;
}
