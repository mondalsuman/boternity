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
    /// Which sub-agent created this memory (None for root agent).
    pub source_agent_id: Option<Uuid>,
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

/// Trust level for shared memories between bots.
///
/// Controls visibility of shared memories. Default is Private.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// Visible to all bots.
    Public,
    /// Visible only to explicitly trusted bots.
    Trusted,
    /// Not shared (default).
    Private,
}

impl Default for TrustLevel {
    fn default() -> Self {
        TrustLevel::Private
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrustLevel::Public => write!(f, "public"),
            TrustLevel::Trusted => write!(f, "trusted"),
            TrustLevel::Private => write!(f, "private"),
        }
    }
}

impl FromStr for TrustLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "public" => Ok(TrustLevel::Public),
            "trusted" => Ok(TrustLevel::Trusted),
            "private" => Ok(TrustLevel::Private),
            other => Err(format!("invalid trust level: '{other}'")),
        }
    }
}

/// A memory entry stored in the vector database for semantic search.
///
/// Linked back to the SQLite `MemoryEntry` via `source_memory_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMemoryEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    /// The extracted fact or preference.
    pub fact: String,
    pub category: MemoryCategory,
    /// Importance score from 1 (low) to 5 (critical).
    pub importance: u8,
    /// Session this memory was extracted from (None for manual memories).
    pub session_id: Option<Uuid>,
    /// Links back to the SQLite MemoryEntry.
    pub source_memory_id: Option<Uuid>,
    /// Name of the embedding model used.
    pub embedding_model: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub access_count: u32,
}

/// A memory entry shared between bots via the shared memory pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryEntry {
    pub id: Uuid,
    /// The shared fact or knowledge.
    pub fact: String,
    pub category: MemoryCategory,
    /// Importance score from 1 (low) to 5 (critical).
    pub importance: u8,
    /// The bot that authored/shared this memory.
    pub author_bot_id: Uuid,
    /// Human-readable name of the authoring bot.
    pub author_bot_name: String,
    /// Visibility level.
    pub trust_level: TrustLevel,
    /// Name of the embedding model used.
    pub embedding_model: String,
    /// SHA-256 hash for tamper detection.
    pub write_hash: String,
    pub created_at: DateTime<Utc>,
}

/// An audit trail entry for memory operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAuditEntry {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub memory_id: Uuid,
    pub action: AuditAction,
    /// Who performed the action: "system", "user", or a bot slug.
    pub actor: String,
    /// Optional JSON context about the action.
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Actions tracked in the memory audit log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditAction {
    Add,
    Delete,
    Share,
    Revoke,
    Merge,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditAction::Add => write!(f, "add"),
            AuditAction::Delete => write!(f, "delete"),
            AuditAction::Share => write!(f, "share"),
            AuditAction::Revoke => write!(f, "revoke"),
            AuditAction::Merge => write!(f, "merge"),
        }
    }
}

impl FromStr for AuditAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "add" => Ok(AuditAction::Add),
            "delete" => Ok(AuditAction::Delete),
            "share" => Ok(AuditAction::Share),
            "revoke" => Ok(AuditAction::Revoke),
            "merge" => Ok(AuditAction::Merge),
            other => Err(format!("invalid audit action: '{other}'")),
        }
    }
}

/// A vector memory entry with search ranking information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedMemory {
    /// The underlying memory entry.
    pub entry: VectorMemoryEntry,
    /// Computed relevance score (higher is more relevant).
    pub relevance_score: f32,
    /// Raw cosine distance from the query embedding.
    pub distance: f32,
    /// Attribution for shared memories (e.g., "Written by BotX").
    pub provenance: Option<String>,
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
            source_agent_id: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"category\":\"preference\""));
        assert!(json.contains("\"importance\":3"));
    }

    #[test]
    fn test_trust_level_roundtrip() {
        for tl in [TrustLevel::Public, TrustLevel::Trusted, TrustLevel::Private] {
            let s = tl.to_string();
            let parsed: TrustLevel = s.parse().unwrap();
            assert_eq!(tl, parsed);
        }
    }

    #[test]
    fn test_trust_level_serde() {
        let tl = TrustLevel::Trusted;
        let json = serde_json::to_string(&tl).unwrap();
        assert_eq!(json, "\"trusted\"");
        let parsed: TrustLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TrustLevel::Trusted);
    }

    #[test]
    fn test_trust_level_default_is_private() {
        assert_eq!(TrustLevel::default(), TrustLevel::Private);
    }

    #[test]
    fn test_audit_action_roundtrip() {
        for action in [
            AuditAction::Add,
            AuditAction::Delete,
            AuditAction::Share,
            AuditAction::Revoke,
            AuditAction::Merge,
        ] {
            let s = action.to_string();
            let parsed: AuditAction = s.parse().unwrap();
            assert_eq!(action, parsed);
        }
    }

    #[test]
    fn test_audit_action_serde() {
        let action = AuditAction::Share;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"share\"");
        let parsed: AuditAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AuditAction::Share);
    }
}
