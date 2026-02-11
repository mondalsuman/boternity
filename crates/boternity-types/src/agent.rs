//! Agent configuration types for Boternity.
//!
//! AgentConfig bundles the identity and LLM settings needed to run
//! a bot as a conversational agent.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for running a bot as a conversational agent.
///
/// Combines bot identity fields (name, slug, emoji) with LLM parameters
/// (model, temperature, max_tokens). Built from a `Bot` + `Identity` at
/// session start time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub bot_id: Uuid,
    pub bot_name: String,
    pub bot_slug: String,
    pub bot_emoji: Option<String>,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_serialize() {
        let config = AgentConfig {
            bot_id: Uuid::now_v7(),
            bot_name: "Luna".to_string(),
            bot_slug: "luna".to_string(),
            bot_emoji: Some("🌙".to_string()),
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"bot_name\":\"Luna\""));
        assert!(json.contains("\"temperature\":0.7"));
    }
}
