//! Filesystem adapters for Boternity.
//!
//! Implements the `FileSystem` trait from `boternity-core` for real filesystem I/O.
//! Also provides helpers for bot directory layout and SOUL.md/IDENTITY.md/USER.md
//! file parsing.

pub mod identity;
pub mod soul;
pub mod user;

use std::path::{Path, PathBuf};

use boternity_core::service::fs::FileSystem;

/// Local filesystem implementation of the `FileSystem` trait.
///
/// All operations go through `tokio::fs` for async I/O.
pub struct LocalFileSystem;

impl LocalFileSystem {
    /// Create a new LocalFileSystem adapter.
    pub fn new() -> Self {
        Self
    }

    /// Compute the bot directory path: `{data_dir}/bots/{slug}/`.
    pub fn bot_dir(data_dir: &Path, slug: &str) -> PathBuf {
        data_dir.join("bots").join(slug)
    }

    /// Compute the SOUL.md path for a bot.
    pub fn soul_path(data_dir: &Path, slug: &str) -> PathBuf {
        Self::bot_dir(data_dir, slug).join("SOUL.md")
    }

    /// Compute the IDENTITY.md path for a bot.
    pub fn identity_path(data_dir: &Path, slug: &str) -> PathBuf {
        Self::bot_dir(data_dir, slug).join("IDENTITY.md")
    }

    /// Compute the USER.md path for a bot.
    pub fn user_path(data_dir: &Path, slug: &str) -> PathBuf {
        Self::bot_dir(data_dir, slug).join("USER.md")
    }
}

impl Default for LocalFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for LocalFileSystem {
    async fn write_file(&self, path: &Path, content: &str) -> Result<(), std::io::Error> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, content).await
    }

    async fn read_file(&self, path: &Path) -> Result<String, std::io::Error> {
        tokio::fs::read_to_string(path).await
    }

    async fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        tokio::fs::create_dir_all(path).await
    }

    async fn exists(&self, path: &Path) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
    }

    async fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        tokio::fs::remove_dir_all(path).await
    }
}

/// Resolve the data directory from environment or platform defaults.
///
/// Priority:
/// 1. `BOTERNITY_DATA_DIR` environment variable
/// 2. Platform-specific data directory (e.g., `~/.boternity` on macOS/Linux)
pub fn resolve_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BOTERNITY_DATA_DIR") {
        return PathBuf::from(dir);
    }

    // Use home directory fallback: ~/.boternity
    if let Some(home) = dirs::home_dir() {
        return home.join(".boternity");
    }

    // Last resort: current directory
    PathBuf::from(".boternity")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_write_and_read_file() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystem::new();
        let file_path = dir.path().join("test.txt");

        fs.write_file(&file_path, "hello world").await.unwrap();
        let content = fs.read_file(&file_path).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystem::new();
        let file_path = dir.path().join("nested").join("deep").join("test.txt");

        fs.write_file(&file_path, "nested content").await.unwrap();
        let content = fs.read_file(&file_path).await.unwrap();
        assert_eq!(content, "nested content");
    }

    #[tokio::test]
    async fn test_create_dir_all() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystem::new();
        let nested = dir.path().join("a").join("b").join("c");

        fs.create_dir_all(&nested).await.unwrap();
        assert!(fs.exists(&nested).await);
    }

    #[tokio::test]
    async fn test_exists() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystem::new();

        assert!(fs.exists(dir.path()).await);
        assert!(!fs.exists(&dir.path().join("nonexistent")).await);
    }

    #[tokio::test]
    async fn test_remove_dir_all() {
        let dir = tempdir().unwrap();
        let fs = LocalFileSystem::new();
        let nested = dir.path().join("to_remove");

        fs.create_dir_all(&nested).await.unwrap();
        fs.write_file(&nested.join("file.txt"), "data").await.unwrap();
        assert!(fs.exists(&nested).await);

        fs.remove_dir_all(&nested).await.unwrap();
        assert!(!fs.exists(&nested).await);
    }

    #[test]
    fn test_bot_dir_paths() {
        let data_dir = PathBuf::from("/home/user/.boternity");
        assert_eq!(
            LocalFileSystem::bot_dir(&data_dir, "luna"),
            PathBuf::from("/home/user/.boternity/bots/luna")
        );
        assert_eq!(
            LocalFileSystem::soul_path(&data_dir, "luna"),
            PathBuf::from("/home/user/.boternity/bots/luna/SOUL.md")
        );
        assert_eq!(
            LocalFileSystem::identity_path(&data_dir, "luna"),
            PathBuf::from("/home/user/.boternity/bots/luna/IDENTITY.md")
        );
        assert_eq!(
            LocalFileSystem::user_path(&data_dir, "luna"),
            PathBuf::from("/home/user/.boternity/bots/luna/USER.md")
        );
    }

    #[test]
    fn test_resolve_data_dir_from_env() {
        // SAFETY: This test is single-threaded and restores the env var immediately.
        unsafe {
            std::env::set_var("BOTERNITY_DATA_DIR", "/tmp/test-boternity");
        }
        let dir = resolve_data_dir();
        assert_eq!(dir, PathBuf::from("/tmp/test-boternity"));
        unsafe {
            std::env::remove_var("BOTERNITY_DATA_DIR");
        }
    }
}
