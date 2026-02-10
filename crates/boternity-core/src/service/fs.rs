//! FileSystem trait for abstracting file I/O.
//!
//! Defined in boternity-core so services can write files without depending on
//! any specific filesystem implementation. The `LocalFileSystem` adapter lives
//! in boternity-infra.

use std::path::Path;

/// Abstraction over filesystem operations.
///
/// This trait allows the service layer to read/write files without coupling
/// to the real filesystem, enabling easy testing with in-memory implementations.
pub trait FileSystem: Send + Sync {
    /// Write string content to a file, creating parent directories as needed.
    fn write_file(
        &self,
        path: &Path,
        content: &str,
    ) -> impl std::future::Future<Output = Result<(), std::io::Error>> + Send;

    /// Read a file's content as a string.
    fn read_file(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<String, std::io::Error>> + Send;

    /// Create a directory and all parent directories.
    fn create_dir_all(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<(), std::io::Error>> + Send;

    /// Check whether a path exists.
    fn exists(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = bool> + Send;

    /// Remove a directory and all its contents.
    fn remove_dir_all(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<(), std::io::Error>> + Send;
}
