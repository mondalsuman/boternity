//! Chat service orchestrating session lifecycle and message persistence.
//!
//! ChatService coordinates between the ChatRepository, MemoryRepository,
//! and AgentEngine to manage the full conversation lifecycle: creating
//! sessions, saving messages, updating titles, and ending sessions.
//!
//! Vector memory operations (search, embed, store, re-embed) are provided
//! as methods that accept `BoxEmbedder` and `BoxVectorMemoryStore` parameters,
//! since the vector backend is optional and not always available.

use boternity_types::chat::{ChatMessage, ChatSession, MessageRole, SessionStatus};
use boternity_types::error::RepositoryError;
use boternity_types::memory::{MemoryEntry, RankedMemory, VectorMemoryEntry};
use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::chat::repository::ChatRepository;
use crate::memory::box_embedder::BoxEmbedder;
use crate::memory::box_vector::BoxVectorMemoryStore;
use crate::memory::store::MemoryRepository;

/// Default number of memories to retrieve per vector search.
const DEFAULT_MEMORY_SEARCH_LIMIT: usize = 10;

/// Default minimum similarity threshold for vector search results.
/// Cosine distance below this is considered too dissimilar.
const DEFAULT_MIN_SIMILARITY: f32 = 0.3;

/// Default cosine distance threshold for semantic deduplication.
/// Memories with distance below this are considered duplicates.
/// Corresponds to ~92.5% similarity.
const DEFAULT_DEDUP_THRESHOLD: f32 = 0.15;

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

    // --- Vector memory operations ---

    /// Search long-term vector memory for facts relevant to a user message.
    ///
    /// Embeds the user message, searches the vector store for semantically
    /// similar memories, and returns ranked results. Called before each LLM
    /// request to populate `AgentContext.recalled_memories`.
    ///
    /// Returns an empty Vec if embedding or search fails (graceful degradation).
    #[tracing::instrument(
        name = "search_memories",
        skip(self, embedder, vector_store, message),
        fields(bot_id = %bot_id, message_len = message.len())
    )]
    pub async fn search_memories_for_message(
        &self,
        bot_id: &Uuid,
        message: &str,
        embedder: &BoxEmbedder,
        vector_store: &BoxVectorMemoryStore,
    ) -> Vec<RankedMemory> {
        // Embed the user message
        let embedding = match embedder.embed(&[message.to_string()]).await {
            Ok(mut embeddings) if !embeddings.is_empty() => embeddings.remove(0),
            Ok(_) => {
                warn!(bot_id = %bot_id, "Embedder returned empty result for message");
                return Vec::new();
            }
            Err(e) => {
                warn!(
                    bot_id = %bot_id,
                    error = %e,
                    "Failed to embed user message for memory search; proceeding without memories"
                );
                return Vec::new();
            }
        };

        // Search vector store
        match vector_store
            .search(bot_id, &embedding, DEFAULT_MEMORY_SEARCH_LIMIT, DEFAULT_MIN_SIMILARITY)
            .await
        {
            Ok(results) => {
                debug!(
                    bot_id = %bot_id,
                    count = results.len(),
                    "Vector memory search returned results"
                );
                results
            }
            Err(e) => {
                warn!(
                    bot_id = %bot_id,
                    error = %e,
                    "Vector memory search failed; proceeding without memories"
                );
                Vec::new()
            }
        }
    }

    /// Embed and store extracted memories in the vector database.
    ///
    /// For each `MemoryEntry`, creates a `VectorMemoryEntry`, embeds the fact
    /// text, checks for semantic duplicates, and stores in the vector DB.
    /// Skips duplicates silently with a debug log.
    ///
    /// Returns the number of memories successfully stored (excluding duplicates).
    #[tracing::instrument(
        name = "embed_and_store_memories",
        skip(self, entries, embedder, vector_store),
        fields(bot_id = %bot_id, entry_count = entries.len())
    )]
    pub async fn embed_and_store_memories(
        &self,
        bot_id: &Uuid,
        entries: &[MemoryEntry],
        embedder: &BoxEmbedder,
        vector_store: &BoxVectorMemoryStore,
    ) -> Result<usize, RepositoryError> {
        if entries.is_empty() {
            return Ok(0);
        }

        // Batch embed all facts
        let texts: Vec<String> = entries.iter().map(|e| e.fact.clone()).collect();
        let embeddings = embedder.embed(&texts).await?;

        if embeddings.len() != entries.len() {
            warn!(
                expected = entries.len(),
                got = embeddings.len(),
                "Embedding count mismatch; aborting store"
            );
            return Err(RepositoryError::Query(format!(
                "Embedding count mismatch: expected {}, got {}",
                entries.len(),
                embeddings.len()
            )));
        }

        let model_name = embedder.model_name().to_string();
        let mut stored_count = 0;

        for (entry, embedding) in entries.iter().zip(embeddings.iter()) {
            // Check for semantic duplicates
            match vector_store
                .check_duplicate(bot_id, embedding, DEFAULT_DEDUP_THRESHOLD)
                .await
            {
                Ok(Some(existing)) => {
                    debug!(
                        bot_id = %bot_id,
                        new_fact = %entry.fact,
                        existing_fact = %existing.fact,
                        "Skipping duplicate memory"
                    );
                    continue;
                }
                Ok(None) => {
                    // No duplicate, proceed to store
                }
                Err(e) => {
                    warn!(
                        bot_id = %bot_id,
                        error = %e,
                        fact = %entry.fact,
                        "Dedup check failed; storing anyway"
                    );
                }
            }

            // Create VectorMemoryEntry from MemoryEntry
            let vector_entry = VectorMemoryEntry {
                id: entry.id,
                bot_id: entry.bot_id,
                fact: entry.fact.clone(),
                category: entry.category.clone(),
                importance: entry.importance,
                session_id: Some(entry.session_id),
                source_memory_id: Some(entry.id),
                embedding_model: model_name.clone(),
                created_at: entry.created_at,
                last_accessed_at: None,
                access_count: 0,
            };

            match vector_store.add(&vector_entry, embedding).await {
                Ok(()) => {
                    stored_count += 1;
                    debug!(
                        bot_id = %bot_id,
                        memory_id = %entry.id,
                        fact = %entry.fact,
                        "Stored memory in vector DB"
                    );
                }
                Err(e) => {
                    warn!(
                        bot_id = %bot_id,
                        memory_id = %entry.id,
                        error = %e,
                        "Failed to store memory in vector DB; continuing with next"
                    );
                }
            }
        }

        info!(
            bot_id = %bot_id,
            stored = stored_count,
            total = entries.len(),
            "Embedded and stored memories in vector DB"
        );

        Ok(stored_count)
    }

    /// Check for embedding model mismatch and re-embed stale memories.
    ///
    /// Compares the current embedder model name against stored entries.
    /// If any entries use a different model, re-embeds them with the current
    /// model and updates the vector store.
    ///
    /// Called at startup / session initialization to detect model changes.
    ///
    /// Returns the number of memories re-embedded.
    #[tracing::instrument(
        name = "check_and_reembed",
        skip(self, embedder, vector_store),
        fields(bot_id = %bot_id)
    )]
    pub async fn check_and_reembed(
        &self,
        bot_id: &Uuid,
        embedder: &BoxEmbedder,
        vector_store: &BoxVectorMemoryStore,
    ) -> Result<usize, RepositoryError> {
        let current_model = embedder.model_name();

        let stale_entries = vector_store
            .get_all_for_reembedding(bot_id, current_model)
            .await?;

        if stale_entries.is_empty() {
            debug!(bot_id = %bot_id, model = current_model, "No stale embeddings found");
            return Ok(0);
        }

        info!(
            bot_id = %bot_id,
            count = stale_entries.len(),
            old_models = ?stale_entries.iter().map(|e| e.embedding_model.as_str()).collect::<std::collections::HashSet<_>>(),
            new_model = current_model,
            "Re-embedding memories with new model"
        );

        // Batch embed all stale facts
        let texts: Vec<String> = stale_entries.iter().map(|e| e.fact.clone()).collect();
        let embeddings = embedder.embed(&texts).await?;

        let mut reembedded_count = 0;

        for (entry, embedding) in stale_entries.iter().zip(embeddings.iter()) {
            match vector_store
                .update_embedding(&entry.id, embedding, current_model)
                .await
            {
                Ok(()) => {
                    reembedded_count += 1;
                }
                Err(e) => {
                    warn!(
                        bot_id = %bot_id,
                        memory_id = %entry.id,
                        error = %e,
                        "Failed to re-embed memory; skipping"
                    );
                }
            }
        }

        info!(
            bot_id = %bot_id,
            reembedded = reembedded_count,
            total = stale_entries.len(),
            "Re-embedding complete"
        );

        Ok(reembedded_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify ChatService is generic over the right traits
    fn _assert_chat_service_generic<C: ChatRepository, M: MemoryRepository>() {
        fn _takes_service<C: ChatRepository, M: MemoryRepository>(_s: &ChatService<C, M>) {}
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_MEMORY_SEARCH_LIMIT, 10);
        assert!(DEFAULT_MIN_SIMILARITY > 0.0 && DEFAULT_MIN_SIMILARITY < 1.0);
        assert!(DEFAULT_DEDUP_THRESHOLD > 0.0 && DEFAULT_DEDUP_THRESHOLD < 1.0);
    }
}
