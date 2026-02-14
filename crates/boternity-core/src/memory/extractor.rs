//! Session memory extraction via LLM.
//!
//! `SessionMemoryExtractor` uses an LLM call to judge what facts, preferences,
//! and decisions from a conversation are worth remembering across sessions.
//! It returns structured `MemoryEntry` objects with category and importance.
//!
//! Failed JSON parsing logs a warning and returns an empty vector -- extraction
//! failures should be queued for retry (via `pending_memory_extractions`), not
//! silently dropped.

use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use boternity_types::chat::ChatMessage;
use boternity_types::llm::{CompletionRequest, LlmError, Message, MessageRole};
use boternity_types::memory::{MemoryCategory, MemoryEntry};

use crate::llm::box_provider::BoxLlmProvider;

/// System prompt for the memory extraction LLM call.
///
/// Instructs the model to extract only information worth remembering across
/// sessions: facts, preferences, decisions, context, and corrections.
const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Extract key facts, preferences, and important points from conversations that would be useful to remember for future conversations with this user.

Rules:
1. Extract ONLY information worth remembering across sessions
2. Each fact must be a single, self-contained sentence
3. Do NOT include greetings, pleasantries, or trivial exchanges
4. Do NOT include information the user explicitly asked to forget
5. Prefer specific facts over vague observations
6. Include the user's name, preferences, and stated goals if mentioned
7. Include decisions made during the conversation
8. Include any corrections the user made (e.g., "Actually, I prefer X not Y")

Return a JSON array. Each element must have exactly these fields:
- "fact": string (one sentence, the key point)
- "category": string (one of: "preference", "fact", "decision", "context", "correction")
- "importance": integer (1-5, where 5 = critical to remember)

If there is nothing worth extracting, return an empty array: []

Example output:
[
  {"fact": "User's name is Alex and they work as a data engineer", "category": "fact", "importance": 5},
  {"fact": "User prefers concise responses without code examples unless asked", "category": "preference", "importance": 4},
  {"fact": "User decided to use PostgreSQL instead of MySQL for the project", "category": "decision", "importance": 3}
]"#;

/// Raw memory entry as returned by the LLM before conversion to `MemoryEntry`.
#[derive(Debug, Deserialize)]
struct RawMemoryEntry {
    fact: String,
    category: String,
    importance: i64,
}

/// Stateless utility for extracting memories from conversation messages.
///
/// Uses an LLM call to identify facts, preferences, decisions, and corrections
/// worth persisting across sessions. Returns structured `MemoryEntry` objects.
pub struct SessionMemoryExtractor;

impl SessionMemoryExtractor {
    /// Extract memory entries from raw LLM `Message` objects.
    ///
    /// Builds a completion request with the extraction prompt, sends it to the
    /// provider at temperature 0.0 (deterministic), and parses the JSON response.
    ///
    /// # Graceful degradation
    /// If JSON parsing fails, a warning is logged and an empty `Vec` is returned.
    /// The caller should queue the extraction for retry rather than silently
    /// dropping it.
    #[tracing::instrument(
        name = "extract_memory",
        skip(provider, messages),
        fields(
            bot_id = %bot_id,
            session_id = %session_id,
            message_count = messages.len(),
        )
    )]
    pub async fn extract(
        provider: &BoxLlmProvider,
        messages: &[Message],
        bot_id: Uuid,
        session_id: Uuid,
    ) -> Result<Vec<MemoryEntry>, LlmError> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }

        let request = CompletionRequest {
            model: String::new(), // Provider uses its default model
            messages: messages.to_vec(),
            system: Some(EXTRACTION_SYSTEM_PROMPT.to_string()),
            max_tokens: 2048,
            temperature: Some(0.0),
            stream: false,
            stop_sequences: None,
            output_config: None,
        };

        let response = provider.complete(&request).await?;

        let raw_content = response.content.trim();

        // Parse the JSON response
        let raw_entries: Vec<RawMemoryEntry> = match serde_json::from_str(raw_content) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    content_preview = &raw_content[..raw_content.len().min(200)],
                    "Failed to parse memory extraction JSON; returning empty result"
                );
                return Ok(Vec::new());
            }
        };

        // Convert raw entries to MemoryEntry with UUIDv7 IDs and clamped importance
        let entries = raw_entries
            .into_iter()
            .filter_map(|raw| {
                let category = match raw.category.to_lowercase().as_str() {
                    "preference" => MemoryCategory::Preference,
                    "fact" => MemoryCategory::Fact,
                    "decision" => MemoryCategory::Decision,
                    "context" => MemoryCategory::Context,
                    "correction" => MemoryCategory::Correction,
                    other => {
                        tracing::warn!(
                            category = other,
                            fact = %raw.fact,
                            "Unknown memory category from LLM; skipping entry"
                        );
                        return None;
                    }
                };

                // Clamp importance to 1..=5
                let importance = raw.importance.clamp(1, 5) as u8;

                Some(MemoryEntry {
                    id: Uuid::now_v7(),
                    bot_id,
                    session_id,
                    fact: raw.fact,
                    category,
                    importance,
                    source_message_id: None,
                    superseded_by: None,
                    created_at: Utc::now(),
                    is_manual: false,
                    source_agent_id: None,
                })
            })
            .collect();

        Ok(entries)
    }

    /// Convenience method: extract memories from `ChatMessage` objects.
    ///
    /// Converts `ChatMessage` to `Message` (dropping chat-specific metadata)
    /// and delegates to [`Self::extract`].
    #[tracing::instrument(
        name = "extract_memory_from_chat",
        skip(provider, messages),
        fields(
            bot_id = %bot_id,
            session_id = %session_id,
            message_count = messages.len(),
        )
    )]
    pub async fn extract_from_messages(
        provider: &BoxLlmProvider,
        messages: &[ChatMessage],
        bot_id: Uuid,
        session_id: Uuid,
    ) -> Result<Vec<MemoryEntry>, LlmError> {
        let llm_messages: Vec<Message> = messages
            .iter()
            .map(|m| Message {
                role: match m.role {
                    boternity_types::llm::MessageRole::System => MessageRole::System,
                    boternity_types::llm::MessageRole::User => MessageRole::User,
                    boternity_types::llm::MessageRole::Assistant => MessageRole::Assistant,
                },
                content: m.content.clone(),
            })
            .collect();

        Self::extract(provider, &llm_messages, bot_id, session_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_memory_entry_deserialize() {
        let json = r#"[
            {"fact": "User prefers dark mode", "category": "preference", "importance": 4},
            {"fact": "User's name is Alex", "category": "fact", "importance": 5}
        ]"#;
        let entries: Vec<RawMemoryEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].fact, "User prefers dark mode");
        assert_eq!(entries[0].category, "preference");
        assert_eq!(entries[0].importance, 4);
    }

    #[test]
    fn test_raw_memory_entry_empty_array() {
        let json = "[]";
        let entries: Vec<RawMemoryEntry> = serde_json::from_str(json).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_importance_clamping() {
        // Simulate what the extract method does for clamping
        let raw_importance: i64 = 10;
        let clamped = raw_importance.clamp(1, 5) as u8;
        assert_eq!(clamped, 5);

        let raw_importance: i64 = -3;
        let clamped = raw_importance.clamp(1, 5) as u8;
        assert_eq!(clamped, 1);

        let raw_importance: i64 = 3;
        let clamped = raw_importance.clamp(1, 5) as u8;
        assert_eq!(clamped, 3);
    }

    #[test]
    fn test_extraction_system_prompt_contains_key_instructions() {
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("Extract ONLY information worth remembering across sessions"));
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("\"fact\""));
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("\"category\""));
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("\"importance\""));
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("empty array: []"));
    }
}
