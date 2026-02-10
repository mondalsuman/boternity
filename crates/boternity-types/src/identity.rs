use serde::{Deserialize, Serialize};

use crate::bot::{BotCategory, BotId};

/// Bot identity configuration (stored in IDENTITY.md).
///
/// Contains system config: LLM settings, visual identity, and organization.
/// All fields have sensible defaults -- only `bot_id` is required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// The bot this identity belongs to.
    pub bot_id: BotId,
    /// Display name shown in UI and CLI output.
    pub display_name: String,
    /// Avatar image path or URL (None for generated avatar).
    pub avatar: Option<String>,
    /// Hex color code for bot's accent color in UI.
    pub accent_color: Option<String>,
    /// Emoji displayed next to bot name in output.
    pub emoji: Option<String>,
    /// LLM model identifier.
    pub model: String,
    /// LLM provider name.
    pub provider: String,
    /// Sampling temperature for LLM responses.
    pub temperature: f64,
    /// Maximum tokens per LLM response.
    pub max_tokens: i32,
    /// System category.
    pub category: BotCategory,
    /// User-managed freeform tags.
    pub tags: Vec<String>,
}

impl Identity {
    /// Default LLM model.
    pub const DEFAULT_MODEL: &'static str = "claude-sonnet-4-20250514";
    /// Default LLM provider.
    pub const DEFAULT_PROVIDER: &'static str = "anthropic";
    /// Default sampling temperature.
    pub const DEFAULT_TEMPERATURE: f64 = 0.7;
    /// Default max tokens per response.
    pub const DEFAULT_MAX_TOKENS: i32 = 4096;

    /// Create a new Identity with sensible defaults.
    pub fn new(bot_id: BotId, display_name: String) -> Self {
        Self {
            bot_id,
            display_name,
            avatar: None,
            accent_color: None,
            emoji: None,
            model: Self::DEFAULT_MODEL.to_string(),
            provider: Self::DEFAULT_PROVIDER.to_string(),
            temperature: Self::DEFAULT_TEMPERATURE,
            max_tokens: Self::DEFAULT_MAX_TOKENS,
            category: BotCategory::default(),
            tags: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_defaults() {
        let id = Identity::new(BotId::new(), "Luna".to_string());
        assert_eq!(id.model, "claude-sonnet-4-20250514");
        assert_eq!(id.provider, "anthropic");
        assert_eq!((id.temperature - 0.7).abs() < f64::EPSILON, true);
        assert_eq!(id.max_tokens, 4096);
        assert_eq!(id.category, BotCategory::Assistant);
    }

    #[test]
    fn test_identity_serde() {
        let id = Identity::new(BotId::new(), "Luna".to_string());
        let json = serde_json::to_string(&id).unwrap();
        let parsed: Identity = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name, "Luna");
        assert_eq!(parsed.model, id.model);
    }
}
