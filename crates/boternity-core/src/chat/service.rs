//! Chat service orchestrating session lifecycle and message persistence.
//!
//! ChatService coordinates between the ChatRepository, MemoryRepository,
//! and AgentEngine to manage the full conversation lifecycle: creating
//! sessions, saving messages, updating titles, and ending sessions.

use boternity_types::chat::{ChatMessage, ChatSession, MessageRole, SessionStatus};
use boternity_types::error::RepositoryError;
use boternity_types::memory::MemoryEntry;
use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::chat::repository::ChatRepository;
use crate::memory::store::MemoryRepository;

/// Orchestrates chat session lifecycle and message persistence.
///
/// Generic over `ChatRepository` and `MemoryRepository` to maintain
/// clean architecture (boternity-core never depends on boternity-infra).
pub struct ChatService<C: ChatRepository, M: MemoryRepository> {
    chat_repo: C,
    memory_repo: M,
}

impl<C: ChatRepository, M: MemoryRepository> ChatService<C, M> {
    /// Create a new chat service with the given repositories.
    pub fn new(chat_repo: C, memory_repo: M) -> Self {
        Self {
            chat_repo,
            memory_repo,
        }
    }

    /// Access the chat repository.
    pub fn chat_repo(&self) -> &C {
        &self.chat_repo
    }

    /// Access the memory repository.
    pub fn memory_repo(&self) -> &M {
        &self.memory_repo
    }

    // --- Session lifecycle ---

    /// Create a new chat session for a bot.
    ///
    /// Initializes a session with Active status and zero token counts.
    pub async fn create_session(
        &self,
        bot_id: Uuid,
        model: String,
    ) -> Result<ChatSession, RepositoryError> {
        let session = ChatSession {
            id: Uuid::now_v7(),
            bot_id,
            title: None,
            started_at: Utc::now(),
            ended_at: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            message_count: 0,
            model,
            status: SessionStatus::Active,
        };

        self.chat_repo.create_session(&session).await
    }

    /// Get a session by ID.
    pub async fn get_session(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<ChatSession>, RepositoryError> {
        self.chat_repo.get_session(session_id).await
    }

    /// List sessions for a bot, ordered by most recent first.
    pub async fn list_sessions(
        &self,
        bot_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ChatSession>, RepositoryError> {
        self.chat_repo.list_sessions(bot_id, limit, offset).await
    }

    /// Update the session title (e.g., auto-generated from first exchange).
    pub async fn update_session_title(
        &self,
        session_id: &Uuid,
        title: String,
    ) -> Result<(), RepositoryError> {
        let session = self.chat_repo.get_session(session_id).await?;
        if let Some(mut session) = session {
            session.title = Some(title);
            self.chat_repo.update_session(&session).await?;
            info!(session_id = %session_id, "Session title updated");
        } else {
            warn!(session_id = %session_id, "Attempted to update title for non-existent session");
        }
        Ok(())
    }

    /// End a session by marking it as completed.
    ///
    /// Sets the status to `Completed` and records the end timestamp.
    pub async fn end_session(
        &self,
        session_id: &Uuid,
    ) -> Result<(), RepositoryError> {
        let session = self.chat_repo.get_session(session_id).await?;
        if let Some(mut session) = session {
            session.status = SessionStatus::Completed;
            session.ended_at = Some(Utc::now());
            self.chat_repo.update_session(&session).await?;
            info!(session_id = %session_id, "Session ended");
        } else {
            warn!(session_id = %session_id, "Attempted to end non-existent session");
        }
        Ok(())
    }

    // --- Message persistence ---

    /// Save a user message to a session.
    ///
    /// Creates a `ChatMessage` with the User role and persists it immediately.
    /// The repository atomically increments the session's message_count.
    pub async fn save_user_message(
        &self,
        session_id: Uuid,
        content: String,
    ) -> Result<ChatMessage, RepositoryError> {
        let message = ChatMessage {
            id: Uuid::now_v7(),
            session_id,
            role: MessageRole::User,
            content,
            created_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            model: None,
            stop_reason: None,
            response_ms: None,
        };

        self.chat_repo.save_message(&message).await?;
        Ok(message)
    }

    /// Save an assistant message to a session.
    ///
    /// Creates a `ChatMessage` with the Assistant role, including token usage
    /// and response timing metadata. Persisted immediately.
    pub async fn save_assistant_message(
        &self,
        session_id: Uuid,
        content: String,
        model: String,
        input_tokens: u32,
        output_tokens: u32,
        stop_reason: String,
        response_ms: u64,
    ) -> Result<ChatMessage, RepositoryError> {
        let message = ChatMessage {
            id: Uuid::now_v7(),
            session_id,
            role: MessageRole::Assistant,
            content,
            created_at: Utc::now(),
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            model: Some(model),
            stop_reason: Some(stop_reason),
            response_ms: Some(response_ms),
        };

        self.chat_repo.save_message(&message).await?;
        Ok(message)
    }

    /// Get messages for a session, ordered by creation time.
    pub async fn get_messages(
        &self,
        session_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ChatMessage>, RepositoryError> {
        self.chat_repo
            .get_messages(session_id, limit, offset)
            .await
    }

    // --- Memory operations ---

    /// Load all memories for a bot (for injection into system prompt).
    ///
    /// Returns memories ordered by importance (highest first).
    pub async fn load_memories(
        &self,
        bot_id: &Uuid,
    ) -> Result<Vec<MemoryEntry>, RepositoryError> {
        self.memory_repo.get_memories(bot_id, None).await
    }

    /// Update the session's token usage counters.
    pub async fn update_session_tokens(
        &self,
        session_id: &Uuid,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<(), RepositoryError> {
        let session = self.chat_repo.get_session(session_id).await?;
        if let Some(mut session) = session {
            session.total_input_tokens += input_tokens;
            session.total_output_tokens += output_tokens;
            self.chat_repo.update_session(&session).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify ChatService is generic over the right traits
    fn _assert_chat_service_generic<C: ChatRepository, M: MemoryRepository>() {
        fn _takes_service<C: ChatRepository, M: MemoryRepository>(_s: &ChatService<C, M>) {}
    }
}
