//! ChatRepository trait definition.
//!
//! Provides CRUD operations for chat sessions, messages, and context summaries.
//! Follows the same RPITIT pattern as BotRepository.

use boternity_types::chat::{ChatMessage, ChatSession, ContextSummary};
use boternity_types::error::RepositoryError;
use uuid::Uuid;

/// Repository trait for chat session and message persistence.
///
/// Implementations live in boternity-infra (e.g., `SqliteChatRepository`).
/// Uses native async fn in traits (RPITIT, Rust 2024 edition).
pub trait ChatRepository: Send + Sync {
    /// Create a new chat session.
    fn create_session(
        &self,
        session: &ChatSession,
    ) -> impl std::future::Future<Output = Result<ChatSession, RepositoryError>> + Send;

    /// Get a chat session by its unique ID.
    fn get_session(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Option<ChatSession>, RepositoryError>> + Send;

    /// Update an existing chat session (e.g., token counts, status).
    fn update_session(
        &self,
        session: &ChatSession,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// List sessions for a bot, ordered by started_at DESC.
    fn list_sessions(
        &self,
        bot_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> impl std::future::Future<Output = Result<Vec<ChatSession>, RepositoryError>> + Send;

    /// Delete a chat session and its messages.
    fn delete_session(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get all active (non-completed, non-crashed) sessions for a bot.
    fn get_active_sessions(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<ChatSession>, RepositoryError>> + Send;

    /// Save a new message within a session.
    fn save_message(
        &self,
        message: &ChatMessage,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get messages for a session, ordered by created_at ASC.
    fn get_messages(
        &self,
        session_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> impl std::future::Future<Output = Result<Vec<ChatMessage>, RepositoryError>> + Send;

    /// Get the total number of messages in a session.
    fn get_message_count(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<u32, RepositoryError>> + Send;

    /// Save a context summary for sliding window management.
    ///
    /// Context summaries belong on ChatRepository (not MemoryRepository)
    /// because they are session-scoped, not bot-scoped.
    fn save_context_summary(
        &self,
        summary: &ContextSummary,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get the most recent context summary for a session.
    fn get_latest_summary(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Option<ContextSummary>, RepositoryError>> + Send;

    /// Clear all messages from a session, resetting message_count to 0.
    ///
    /// Keeps the session record intact but removes all chat_messages.
    fn clear_messages(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Count total sessions across all bots.
    fn count_sessions(
        &self,
    ) -> impl std::future::Future<Output = Result<u64, RepositoryError>> + Send;

    /// Count total messages across all sessions.
    fn count_messages(
        &self,
    ) -> impl std::future::Future<Output = Result<u64, RepositoryError>> + Send;
}
