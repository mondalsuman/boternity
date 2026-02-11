//! Memory types for Boternity.
//!
//! These types model the bot's long-term memory: extracted facts,
//! preferences, and decisions that persist across conversations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::fmt;
use std::str::FromStr;

/// Category of a memory entry.
///
/// Used to classify extracted memories for retrieval prioritization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryCategory {
    Preference,
    Fact,
    Decision,
    Context,
    Correction,
}

impl fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryCategory::Preference => write!(f, "preference"),
            MemoryCategory::Fact => write!(f, "fact"),
            MemoryCategory::Decision => write!(f, "decision"),
            MemoryCategory::Context => write!(f, "context"),
            MemoryCategory::Correction => write!(f, "correction"),
        }
    }
}

impl FromStr for MemoryCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "preference" => Ok(MemoryCategory::Preference),
            "fact" => Ok(MemoryCategory::Fact),
            "decision" => Ok(MemoryCategory::Decision),
            "context" => Ok(MemoryCategory::Context),
            "correction" => Ok(MemoryCategory::Correction),
            other => Err(format!("invalid memory category: '{other}'")),
        }
    }
}

/// A single memory entry extracted from a conversation.
///
/// Memories are bot-scoped and session-linked. They can be superseded
/// by newer memories (e.g., user corrects a preference).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub session_id: Uuid,
    /// The extracted fact or preference.
    pub fact: String,
    pub category: MemoryCategory,
    /// Importance score from 1 (low) to 5 (critical).
    pub importance: u8,
    /// The message that triggered this memory extraction.
    pub source_message_id: Option<Uuid>,
    /// If this memory was superseded by a newer one, its ID.
    pub superseded_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// Whether this memory was manually created by the user.
    pub is_manual: bool,
}

/// A pending memory extraction job.
///
/// Created when a session ends or reaches a summarization point.
/// The extraction service processes these asynchronously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingExtraction {
    pub id: Uuid,
    pub session_id: Uuid,
    pub bot_id: Uuid,
    pub attempt_count: u32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub next_attempt_at: DateTime<Utc>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_category_roundtrip() {
        for cat in [
            MemoryCategory::Preference,
            MemoryCategory::Fact,
            MemoryCategory::Decision,
            MemoryCategory::Context,
            MemoryCategory::Correction,
        ] {
            let s = cat.to_string();
            let parsed: MemoryCategory = s.parse().unwrap();
            assert_eq!(cat, parsed);
        }
    }

    #[test]
    fn test_memory_category_serde() {
        let cat = MemoryCategory::Preference;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"preference\"");
        let parsed: MemoryCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, MemoryCategory::Preference);
    }

    #[test]
    fn test_memory_entry_serialize() {
        let entry = MemoryEntry {
            id: Uuid::now_v7(),
            bot_id: Uuid::now_v7(),
            session_id: Uuid::now_v7(),
            fact: "User prefers dark mode".to_string(),
            category: MemoryCategory::Preference,
            importance: 3,
            source_message_id: None,
            superseded_by: None,
            created_at: Utc::now(),
            is_manual: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"category\":\"preference\""));
        assert!(json.contains("\"importance\":3"));
    }
}
