//! Soul repository trait definition.

use boternity_types::bot::BotId;
use boternity_types::error::RepositoryError;
use boternity_types::soul::{Soul, SoulVersion};

/// Repository trait for soul (SOUL.md) version persistence.
///
/// Each soul version is immutable once saved. The repository tracks the
/// full version history per bot.
pub trait SoulRepository: Send + Sync {
    /// Save a new soul version. Returns the saved soul with version number.
    fn save_version(
        &self,
        soul: &Soul,
    ) -> impl std::future::Future<Output = Result<Soul, RepositoryError>> + Send;

    /// Get the current (latest) soul for a bot.
    fn get_current(
        &self,
        bot_id: &BotId,
    ) -> impl std::future::Future<Output = Result<Option<Soul>, RepositoryError>> + Send;

    /// Get a specific version of a bot's soul.
    fn get_version(
        &self,
        bot_id: &BotId,
        version: i32,
    ) -> impl std::future::Future<Output = Result<Option<Soul>, RepositoryError>> + Send;

    /// List all versions of a bot's soul (for history/diffing).
    fn list_versions(
        &self,
        bot_id: &BotId,
    ) -> impl std::future::Future<Output = Result<Vec<SoulVersion>, RepositoryError>> + Send;

    /// Get the stored SHA-256 hash of the current soul (for integrity checks).
    fn get_stored_hash(
        &self,
        bot_id: &BotId,
    ) -> impl std::future::Future<Output = Result<Option<String>, RepositoryError>> + Send;
}
