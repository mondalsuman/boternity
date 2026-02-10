use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::fmt;
use std::str::FromStr;

/// Unique identifier for a bot, wrapping a UUID v7 (time-sortable).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BotId(pub Uuid);

impl BotId {
    /// Create a new BotId using UUID v7 (time-sortable, guaranteed ordering).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Create a BotId from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for BotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for BotId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// A bot in the Boternity platform.
///
/// Each bot has a distinct identity (SOUL.md), configuration (IDENTITY.md),
/// and user briefing (USER.md). Bots are managed via CLI or REST API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bot {
    pub id: BotId,
    /// URL-safe unique slug derived from name ("Research Assistant" -> "research-assistant").
    pub slug: String,
    /// Freeform display name (duplicates allowed across bots).
    pub name: String,
    /// Short description (1-2 sentences for listings).
    pub description: String,
    /// Current lifecycle state.
    pub status: BotStatus,
    /// System category for organization.
    pub category: BotCategory,
    /// User-managed freeform tags.
    pub tags: Vec<String>,
    /// Reserved for future multi-user support.
    pub user_id: Option<String>,
    /// Total number of conversations this bot has had.
    pub conversation_count: i64,
    /// Total tokens consumed across all conversations.
    pub total_tokens_used: i64,
    /// Number of soul versions created.
    pub version_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Last time this bot was used in a conversation.
    pub last_active_at: Option<DateTime<Utc>>,
}

/// Bot lifecycle states.
///
/// - Active: fully functional, can chat
/// - Disabled: paused, visible but cannot chat
/// - Archived: hidden from default views, all data preserved, restorable
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BotStatus {
    Active,
    Disabled,
    Archived,
}

impl fmt::Display for BotStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BotStatus::Active => write!(f, "active"),
            BotStatus::Disabled => write!(f, "disabled"),
            BotStatus::Archived => write!(f, "archived"),
        }
    }
}

impl FromStr for BotStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(BotStatus::Active),
            "disabled" => Ok(BotStatus::Disabled),
            "archived" => Ok(BotStatus::Archived),
            other => Err(format!("invalid bot status: '{other}'")),
        }
    }
}

impl Default for BotStatus {
    fn default() -> Self {
        BotStatus::Active
    }
}

/// System categories for bot organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BotCategory {
    Assistant,
    Creative,
    Research,
    Utility,
}

impl fmt::Display for BotCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BotCategory::Assistant => write!(f, "assistant"),
            BotCategory::Creative => write!(f, "creative"),
            BotCategory::Research => write!(f, "research"),
            BotCategory::Utility => write!(f, "utility"),
        }
    }
}

impl FromStr for BotCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "assistant" => Ok(BotCategory::Assistant),
            "creative" => Ok(BotCategory::Creative),
            "research" => Ok(BotCategory::Research),
            "utility" => Ok(BotCategory::Utility),
            other => Err(format!("invalid bot category: '{other}'")),
        }
    }
}

impl Default for BotCategory {
    fn default() -> Self {
        BotCategory::Assistant
    }
}

/// Request to create a new bot. Only `name` is required -- everything else
/// gets sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBotRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<BotCategory>,
    pub tags: Option<Vec<String>>,
}

/// Generate a URL-safe slug from a display name.
///
/// Rules:
/// - Lowercase
/// - Replace non-alphanumeric characters with hyphens
/// - Collapse consecutive hyphens into one
/// - Trim leading/trailing hyphens
///
/// # Examples
///
/// ```
/// use boternity_types::bot::slugify;
///
/// assert_eq!(slugify("Research Assistant"), "research-assistant");
/// assert_eq!(slugify("My  Cool  Bot!"), "my-cool-bot");
/// assert_eq!(slugify("---hello---world---"), "hello-world");
/// ```
pub fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens and trim edges
    let mut result = String::with_capacity(slug.len());
    let mut prev_was_hyphen = true; // treat start as hyphen to trim leading
    for c in slug.chars() {
        if c == '-' {
            if !prev_was_hyphen {
                result.push('-');
            }
            prev_was_hyphen = true;
        } else {
            result.push(c);
            prev_was_hyphen = false;
        }
    }

    // Trim trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Research Assistant"), "research-assistant");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("My  Cool  Bot!"), "my-cool-bot");
    }

    #[test]
    fn test_slugify_leading_trailing() {
        assert_eq!(slugify("---hello---world---"), "hello-world");
    }

    #[test]
    fn test_slugify_single_word() {
        assert_eq!(slugify("Luna"), "luna");
    }

    #[test]
    fn test_slugify_numbers() {
        assert_eq!(slugify("Bot v2.0"), "bot-v2-0");
    }

    #[test]
    fn test_bot_id_display() {
        let id = BotId::new();
        let s = id.to_string();
        let parsed: BotId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_bot_status_roundtrip() {
        for status in [BotStatus::Active, BotStatus::Disabled, BotStatus::Archived] {
            let s = status.to_string();
            let parsed: BotStatus = s.parse().unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_bot_category_roundtrip() {
        for cat in [
            BotCategory::Assistant,
            BotCategory::Creative,
            BotCategory::Research,
            BotCategory::Utility,
        ] {
            let s = cat.to_string();
            let parsed: BotCategory = s.parse().unwrap();
            assert_eq!(cat, parsed);
        }
    }
}
