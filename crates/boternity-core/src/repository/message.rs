//! Message repository trait definition.
//!
//! Defines the storage interface for bot-to-bot messages, pub/sub channels,
//! and channel subscriptions. The infrastructure layer (boternity-infra)
//! implements this trait with SQLite persistence.

use boternity_types::error::RepositoryError;
use boternity_types::message::{BotMessage, BotSubscription, Channel};
use uuid::Uuid;

/// Repository trait for bot-to-bot message persistence.
///
/// Covers three entity families:
/// - **Messages:** Save and query bot-to-bot messages (direct + channel).
/// - **Channels:** Create and list pub/sub channels.
/// - **Subscriptions:** Manage bot-channel subscriptions (persist for restart recovery).
///
/// Uses native async fn in traits (Rust 2024 edition, no async_trait macro).
pub trait MessageRepository: Send + Sync {
    // -----------------------------------------------------------------------
    // Messages
    // -----------------------------------------------------------------------

    /// Persist a bot-to-bot message for audit trail.
    fn save_message(
        &self,
        msg: &BotMessage,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get messages between two bots (in either direction), ordered by timestamp DESC.
    fn get_messages_between(
        &self,
        bot_a: &Uuid,
        bot_b: &Uuid,
        limit: u32,
    ) -> impl std::future::Future<Output = Result<Vec<BotMessage>, RepositoryError>> + Send;

    /// Get messages published to a channel, ordered by timestamp DESC.
    fn get_channel_messages(
        &self,
        channel: &str,
        limit: u32,
    ) -> impl std::future::Future<Output = Result<Vec<BotMessage>, RepositoryError>> + Send;

    // -----------------------------------------------------------------------
    // Channels
    // -----------------------------------------------------------------------

    /// List all known channels.
    fn list_channels(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Channel>, RepositoryError>> + Send;

    /// Create a new channel. Returns `Conflict` if the name already exists.
    fn create_channel(
        &self,
        channel: &Channel,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    // -----------------------------------------------------------------------
    // Subscriptions
    // -----------------------------------------------------------------------

    /// Subscribe a bot to a channel (idempotent -- no error if already subscribed).
    fn subscribe(
        &self,
        sub: &BotSubscription,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Unsubscribe a bot from a channel. Returns `true` if the subscription existed.
    fn unsubscribe(
        &self,
        bot_id: &Uuid,
        channel_name: &str,
    ) -> impl std::future::Future<Output = Result<bool, RepositoryError>> + Send;

    /// Get all subscriptions for a given bot.
    fn get_subscriptions(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<BotSubscription>, RepositoryError>> + Send;

    /// Get all bot IDs subscribed to a given channel.
    fn get_channel_subscribers(
        &self,
        channel_name: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Uuid>, RepositoryError>> + Send;
}
