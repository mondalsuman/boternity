use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bot::BotId;

use std::fmt;
use std::str::FromStr;

/// Unique identifier for a soul entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SoulId(pub Uuid);

impl SoulId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for SoulId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SoulId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SoulId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// A versioned soul (SOUL.md content) for a bot.
///
/// Each soul version is immutable once created. The hash is SHA-256 of the
/// content, used for integrity verification at bot startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soul {
    pub id: SoulId,
    /// The bot this soul belongs to.
    pub bot_id: BotId,
    /// Full SOUL.md content (YAML frontmatter + markdown body).
    pub content: String,
    /// SHA-256 hex digest of the content.
    pub hash: String,
    /// Monotonically increasing version number per bot.
    pub version: i32,
    /// Optional commit message describing what changed in this version.
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Result of a soul integrity verification check.
///
/// Compares the SHA-256 hash of the SOUL.md file on disk against the stored
/// hash in the database. A mismatch indicates the file was modified outside
/// of the Boternity update flow (potential tampering).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulIntegrityResult {
    /// Whether the file hash matches the stored hash.
    pub valid: bool,
    /// The expected hash (from database).
    pub expected_hash: String,
    /// The actual hash (computed from file on disk).
    pub actual_hash: String,
    /// The current version number.
    pub version: i32,
}

/// Structured data parsed from the YAML frontmatter of SOUL.md.
///
/// Format:
/// ```yaml
/// ---
/// name: Luna
/// traits: [curious, empathetic, analytical]
/// tone: warm and conversational
/// ---
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulFrontmatter {
    /// Bot's personality name.
    pub name: String,
    /// Personality trait keywords.
    pub traits: Vec<String>,
    /// Communication tone descriptor.
    pub tone: String,
}

/// A version entry in the soul's history, used for listing and diffing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulVersion {
    /// Version number (1-based, monotonically increasing per bot).
    pub version: i32,
    /// SHA-256 hex digest of the content.
    pub hash: String,
    /// Full content snapshot for this version.
    pub content: String,
    pub created_at: DateTime<Utc>,
    /// Optional commit message describing what changed.
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soul_id_display_parse() {
        let id = SoulId::new();
        let s = id.to_string();
        let parsed: SoulId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_soul_frontmatter_serde() {
        let fm = SoulFrontmatter {
            name: "Luna".to_string(),
            traits: vec!["curious".to_string(), "empathetic".to_string()],
            tone: "warm and conversational".to_string(),
        };
        let json = serde_json::to_string(&fm).unwrap();
        let parsed: SoulFrontmatter = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Luna");
        assert_eq!(parsed.traits.len(), 2);
    }
}
