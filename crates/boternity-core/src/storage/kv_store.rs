//! Key-value store trait.
//!
//! Defines the interface for bot-scoped key-value storage.
//! Implementations live in boternity-infra.

use boternity_types::error::RepositoryError;
use boternity_types::storage::KvEntry;
use uuid::Uuid;

/// Trait for bot-scoped key-value persistent storage.
///
/// Stores arbitrary JSON values keyed by bot ID and string key.
/// Uses RPITIT (native async fn in traits, Rust 2024 edition).
/// Implementations live in boternity-infra.
pub trait KvStore: Send + Sync {
    /// Get a value by key. Returns None if the key does not exist.
    fn get(
        &self,
        bot_id: &Uuid,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<serde_json::Value>, RepositoryError>> + Send;

    /// Set a value for a key (upsert).
    fn set(
        &self,
        bot_id: &Uuid,
        key: &str,
        value: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete a key. No-op if key does not exist.
    fn delete(
        &self,
        bot_id: &Uuid,
        key: &str,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// List all keys for a bot.
    fn list_keys(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<String>, RepositoryError>> + Send;

    /// Get the full entry including timestamps.
    fn get_entry(
        &self,
        bot_id: &Uuid,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<KvEntry>, RepositoryError>> + Send;
}
