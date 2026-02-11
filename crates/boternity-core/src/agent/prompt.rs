//! System prompt builder for Boternity agents.
//!
//! Assembles the system prompt from personality files (SOUL.md, IDENTITY.md,
//! USER.md) and session memories using XML tag boundaries for clear section
//! delineation.

use boternity_types::agent::AgentConfig;
use boternity_types::memory::MemoryEntry;

/// Builds a system prompt from bot personality files and memories.
///
/// The prompt uses XML tags for section boundaries so the LLM can
/// distinguish between soul, identity, user context, and memory sections.
///
/// Layout:
/// ```text
/// <soul>{soul_content}</soul>
/// <identity>Name: ... Emoji: ... Model: ...</identity>
/// <user_context>{user_md_content}</user_context>
/// <session_memory>Key points from previous conversations: ...</session_memory>
/// <instructions>You are {name}. Always stay in character...</instructions>
/// ```
pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    /// Build the complete system prompt from all personality sources.
    ///
    /// Sections are wrapped in XML tags for clear delineation:
    /// - `<soul>`: The bot's core personality from SOUL.md
    /// - `<identity>`: Name, emoji, and model from IDENTITY.md config
    /// - `<user_context>`: Standing instructions from USER.md
    /// - `<session_memory>`: Extracted facts from previous conversations
    /// - `<instructions>`: Behavioral guidelines
    pub fn build(
        config: &AgentConfig,
        soul: &str,
        identity: &str,
        user: &str,
        memories: &[MemoryEntry],
    ) -> String {
        let mut sections = Vec::with_capacity(5);

        // Soul section -- the bot's core personality
        if !soul.trim().is_empty() {
            sections.push(format!("<soul>\n{}\n</soul>", soul.trim()));
        }

        // Identity section -- structured config from IDENTITY.md
        if !identity.trim().is_empty() {
            sections.push(format!("<identity>\n{}\n</identity>", identity.trim()));
        } else {
            // Fallback: build identity from AgentConfig fields
            let emoji_line = config
                .bot_emoji
                .as_deref()
                .map(|e| format!("\nEmoji: {e}"))
                .unwrap_or_default();
            sections.push(format!(
                "<identity>\nName: {}{emoji_line}\nModel: {}\n</identity>",
                config.bot_name, config.model
            ));
        }

        // User context section -- standing instructions from USER.md
        if !user.trim().is_empty() {
            sections.push(format!(
                "<user_context>\n{}\n</user_context>",
                user.trim()
            ));
        }

        // Session memory section -- extracted facts from previous conversations
        if !memories.is_empty() {
            let memory_lines: Vec<String> = memories
                .iter()
                .map(|m| format!("- [{}] {}", m.category, m.fact))
                .collect();
            sections.push(format!(
                "<session_memory>\nKey points from previous conversations:\n{}\n</session_memory>",
                memory_lines.join("\n")
            ));
        }

        // Instructions section -- behavioral guidelines
        sections.push(format!(
            "<instructions>\n\
            You are {}. Always stay in character as defined in your soul.\n\
            Express your personality strongly in every response.\n\
            Reference past conversations naturally without saying \"I remember\".\n\
            When uncertain, acknowledge it honestly.\n\
            </instructions>",
            config.bot_name
        ));

        sections.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::memory::MemoryCategory;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_config() -> AgentConfig {
        AgentConfig {
            bot_id: Uuid::now_v7(),
            bot_name: "Luna".to_string(),
            bot_slug: "luna".to_string(),
            bot_emoji: Some("🌙".to_string()),
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }

    fn test_memory(fact: &str, category: MemoryCategory) -> MemoryEntry {
        MemoryEntry {
            id: Uuid::now_v7(),
            bot_id: Uuid::now_v7(),
            session_id: Uuid::now_v7(),
            fact: fact.to_string(),
            category,
            importance: 3,
            source_message_id: None,
            superseded_by: None,
            created_at: Utc::now(),
            is_manual: false,
        }
    }

    #[test]
    fn test_build_full_prompt() {
        let config = test_config();
        let soul = "I am a creative writing assistant.";
        let identity = "Name: Luna\nEmoji: 🌙\nModel: claude-sonnet-4-20250514";
        let user = "Please keep responses concise.";
        let memories = vec![
            test_memory("User prefers dark mode", MemoryCategory::Preference),
            test_memory("User is a Rust developer", MemoryCategory::Fact),
        ];

        let prompt = SystemPromptBuilder::build(&config, soul, identity, user, &memories);

        assert!(prompt.contains("<soul>"));
        assert!(prompt.contains("</soul>"));
        assert!(prompt.contains("<identity>"));
        assert!(prompt.contains("</identity>"));
        assert!(prompt.contains("<user_context>"));
        assert!(prompt.contains("</user_context>"));
        assert!(prompt.contains("<session_memory>"));
        assert!(prompt.contains("</session_memory>"));
        assert!(prompt.contains("<instructions>"));
        assert!(prompt.contains("</instructions>"));
        assert!(prompt.contains("User prefers dark mode"));
        assert!(prompt.contains("User is a Rust developer"));
        assert!(prompt.contains("You are Luna"));
    }

    #[test]
    fn test_build_without_memories() {
        let config = test_config();
        let prompt =
            SystemPromptBuilder::build(&config, "Soul content", "Identity content", "User context", &[]);

        assert!(prompt.contains("<soul>"));
        assert!(prompt.contains("<identity>"));
        assert!(prompt.contains("<user_context>"));
        assert!(!prompt.contains("<session_memory>"));
        assert!(prompt.contains("<instructions>"));
    }

    #[test]
    fn test_build_empty_identity_uses_config_fallback() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build(&config, "Soul content", "", "", &[]);

        assert!(prompt.contains("Name: Luna"));
        assert!(prompt.contains("Emoji: 🌙"));
        assert!(prompt.contains("Model: claude-sonnet-4-20250514"));
        assert!(!prompt.contains("<user_context>"));
    }

    #[test]
    fn test_build_empty_soul_omits_section() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build(&config, "", "Identity", "", &[]);

        assert!(!prompt.contains("<soul>"));
        assert!(prompt.contains("<identity>"));
    }

    #[test]
    fn test_memory_format_includes_category() {
        let config = test_config();
        let memories = vec![test_memory("Likes cats", MemoryCategory::Preference)];

        let prompt = SystemPromptBuilder::build(&config, "Soul", "Identity", "", &memories);

        assert!(prompt.contains("- [preference] Likes cats"));
    }
}
