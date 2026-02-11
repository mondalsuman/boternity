//! Chat session, message, and context summary types for Boternity.
//!
//! These types model chat conversations between users and bots:
//! sessions, messages, and context summaries for sliding window management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::fmt;
use std::str::FromStr;

// Re-export MessageRole from llm module (it's used in both chat and llm contexts).
pub use crate::llm::MessageRole;

/// Lifecycle status of a chat session.
///
/// Maps to the CHECK constraint in the SQLite schema:
/// `CHECK (status IN ('active', 'completed', 'crashed'))`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Completed,
    Crashed,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionStatus::Active => write!(f, "active"),
            SessionStatus::Completed => write!(f, "completed"),
            SessionStatus::Crashed => write!(f, "crashed"),
        }
    }
}

impl FromStr for SessionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(SessionStatus::Active),
            "completed" => Ok(SessionStatus::Completed),
            "crashed" => Ok(SessionStatus::Crashed),
            other => Err(format!("invalid session status: '{other}'")),
        }
    }
}

impl Default for SessionStatus {
    fn default() -> Self {
        SessionStatus::Active
    }
}

/// A chat session between a user and a bot.
///
/// Each session tracks its lifetime, token usage, and message count.
/// Sessions belong to a single bot (identified by `bot_id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub title: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub message_count: u32,
    pub model: String,
    pub status: SessionStatus,
}

/// A single message within a chat session.
///
/// Messages are ordered by `created_at` within a session.
/// Assistant messages include token usage and response timing metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
    /// Input tokens consumed by this message (assistant messages only).
    pub input_tokens: Option<u32>,
    /// Output tokens generated for this message (assistant messages only).
    pub output_tokens: Option<u32>,
    /// Model used for this message (assistant messages only).
    pub model: Option<String>,
    /// Why the LLM stopped generating (assistant messages only).
    pub stop_reason: Option<String>,
    /// Response latency in milliseconds (assistant messages only).
    pub response_ms: Option<u64>,
}

/// A summary of a range of messages within a chat session.
///
/// Used by the sliding window context manager to compress older messages
/// into summaries, freeing token budget for recent conversation.
/// This is the definitive location for this type -- context summaries
/// are scoped to chat sessions, not memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub id: Uuid,
    pub session_id: Uuid,
    pub summary: String,
    /// First message index covered by this summary.
    pub messages_start: u32,
    /// Last message index covered by this summary.
    pub messages_end: u32,
    /// Approximate token count of this summary.
    pub token_count: u32,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_status_roundtrip() {
        for status in [
            SessionStatus::Active,
            SessionStatus::Completed,
            SessionStatus::Crashed,
        ] {
            let s = status.to_string();
            let parsed: SessionStatus = s.parse().unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_session_status_serde() {
        let status = SessionStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: SessionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SessionStatus::Active);
    }

    #[test]
    fn test_session_status_default() {
        assert_eq!(SessionStatus::default(), SessionStatus::Active);
    }

    #[test]
    fn test_message_role_reexport() {
        // Verify MessageRole is accessible from the chat module.
        let role = MessageRole::User;
        assert_eq!(role.to_string(), "user");
    }

    #[test]
    fn test_chat_session_serialize() {
        let session = ChatSession {
            id: Uuid::now_v7(),
            bot_id: Uuid::now_v7(),
            title: Some("Test chat".to_string()),
            started_at: Utc::now(),
            ended_at: None,
            total_input_tokens: 100,
            total_output_tokens: 200,
            message_count: 5,
            model: "claude-sonnet-4-20250514".to_string(),
            status: SessionStatus::Active,
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"status\":\"active\""));
    }
}
