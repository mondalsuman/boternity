//! System prompt builder for Boternity agents.
//!
//! Assembles the system prompt from personality files (SOUL.md, IDENTITY.md,
//! USER.md), session memories, and long-term vector memories using XML tag
//! boundaries for clear section delineation.

use std::path::PathBuf;

use boternity_types::agent::AgentConfig;
use boternity_types::memory::{MemoryEntry, RankedMemory};
use boternity_types::skill::SkillManifest;

use crate::skill::prompt_injector;

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
/// <long_term_memory>Semantically recalled facts from past interactions: ...</long_term_memory>
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
    /// - `<long_term_memory>`: Semantically recalled facts from vector search
    /// - `<instructions>`: Behavioral guidelines
    pub fn build(
        config: &AgentConfig,
        soul: &str,
        identity: &str,
        user: &str,
        memories: &[MemoryEntry],
        recalled_memories: &[RankedMemory],
    ) -> String {
        let mut sections = Vec::with_capacity(7);

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

        // Long-term memory section -- semantically recalled vector memories
        if !recalled_memories.is_empty() {
            let memory_lines: Vec<String> = recalled_memories
                .iter()
                .map(|rm| Self::format_recalled_memory(rm))
                .collect();
            sections.push(format!(
                "<long_term_memory>\n\
                Things you know about the user from past interactions:\n\
                {}\n\
                </long_term_memory>",
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

    /// Build the complete system prompt with agent spawning capabilities.
    ///
    /// Same as [`build()`] but appends an `<agent_capabilities>` section that
    /// teaches the LLM how to spawn sub-agents via XML blocks. Used for the
    /// root agent (depth 0) when agent hierarchy is enabled.
    pub fn build_with_capabilities(
        config: &AgentConfig,
        soul: &str,
        identity: &str,
        user: &str,
        memories: &[MemoryEntry],
        recalled_memories: &[RankedMemory],
    ) -> String {
        let base = Self::build(config, soul, identity, user, memories, recalled_memories);
        format!("{base}\n\n{}", Self::agent_capabilities_section())
    }

    /// Build the complete system prompt with skill sections.
    ///
    /// Same as [`build()`] but adds skill metadata and active skill prompts
    /// using the prompt injector. `<available_skills>` provides Level 1
    /// disclosure (skill names + descriptions), and `<active_skills>` provides
    /// Level 2 disclosure (full skill bodies injected after `</identity>`).
    ///
    /// When both `all_skills` and `active_skills` are empty, the result is
    /// identical to [`build()`].
    pub fn build_with_skills(
        config: &AgentConfig,
        soul: &str,
        identity: &str,
        user: &str,
        memories: &[MemoryEntry],
        recalled_memories: &[RankedMemory],
        all_skills: &[(SkillManifest, PathBuf)],
        active_skills: &[(SkillManifest, String)],
    ) -> String {
        let base = Self::build(config, soul, identity, user, memories, recalled_memories);
        prompt_injector::build_skill_enhanced_prompt(&base, all_skills, active_skills)
    }

    /// Build a focused system prompt for a sub-agent executing a specific task.
    ///
    /// Includes the bot's soul and identity (sub-agents stay in character) plus
    /// a `<task>` section and `<sub_agent_instructions>`. Does NOT include
    /// user_context, session_memory, or long_term_memory (sub-agents get fresh
    /// context).
    ///
    /// If `depth < 3`, the `<agent_capabilities>` section is included to allow
    /// recursive spawning (sub-agents can spawn their own sub-agents).
    pub fn build_for_sub_agent(
        config: &AgentConfig,
        soul: &str,
        identity: &str,
        task: &str,
        depth: u8,
    ) -> String {
        let mut sections = Vec::with_capacity(6);

        // Soul section -- sub-agents stay in character
        if !soul.trim().is_empty() {
            sections.push(format!("<soul>\n{}\n</soul>", soul.trim()));
        }

        // Identity section
        if !identity.trim().is_empty() {
            sections.push(format!("<identity>\n{}\n</identity>", identity.trim()));
        } else {
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

        // Task section -- the specific task this sub-agent is focused on
        sections.push(format!("<task>\n{}\n</task>", task.trim()));

        // Sub-agent instructions
        sections.push(
            "<sub_agent_instructions>\n\
            You are executing a focused sub-task. Respond with your result directly. \
            Be thorough but stay focused on the task.\n\
            </sub_agent_instructions>"
                .to_string(),
        );

        // Agent capabilities -- only if recursive spawning is allowed (depth < 3)
        if depth < 3 {
            sections.push(Self::agent_capabilities_section());
        }

        sections.join("\n\n")
    }

    /// The `<agent_capabilities>` XML section content.
    ///
    /// Instructs the LLM on how to spawn sub-agents using `<spawn_agents>` blocks.
    fn agent_capabilities_section() -> String {
        "<agent_capabilities>\n\
        You can decompose complex tasks by spawning sub-agents. To do this, include a spawn block in your response:\n\
        \n\
        For parallel execution (tasks run simultaneously):\n\
        <spawn_agents mode=\"parallel\">\n  \
          <agent task=\"Description of sub-task 1\" />\n  \
          <agent task=\"Description of sub-task 2\" />\n\
        </spawn_agents>\n\
        \n\
        For sequential execution (each task sees the previous result):\n\
        <spawn_agents mode=\"sequential\">\n  \
          <agent task=\"First step description\" />\n  \
          <agent task=\"Second step description\" />\n\
        </spawn_agents>\n\
        \n\
        Guidelines:\n\
        - Only spawn sub-agents when the task genuinely benefits from decomposition\n\
        - Each task description should be specific and self-contained\n\
        - Sub-agents inherit your personality and respond in character\n\
        - You may include text before the spawn block to explain your approach\n\
        - After sub-agents complete, you will receive their results and should synthesize a cohesive response\n\
        - Sub-agents can spawn their own sub-agents up to 3 levels deep\n\
        </agent_capabilities>"
            .to_string()
    }

    /// Format a single recalled memory for the system prompt.
    ///
    /// Outputs natural-language facts without scores or metadata.
    /// Shared memories include provenance annotation.
    fn format_recalled_memory(rm: &RankedMemory) -> String {
        match &rm.provenance {
            Some(prov) => format!("- {} ({})", rm.entry.fact, prov),
            None => format!("- {}", rm.entry.fact),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::memory::{MemoryCategory, VectorMemoryEntry};
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
            source_agent_id: None,
        }
    }

    fn test_ranked_memory(fact: &str, category: MemoryCategory, provenance: Option<&str>) -> RankedMemory {
        RankedMemory {
            entry: VectorMemoryEntry {
                id: Uuid::now_v7(),
                bot_id: Uuid::now_v7(),
                fact: fact.to_string(),
                category,
                importance: 4,
                session_id: Some(Uuid::now_v7()),
                source_memory_id: Some(Uuid::now_v7()),
                embedding_model: "all-MiniLM-L6-v2".to_string(),
                created_at: Utc::now(),
                last_accessed_at: None,
                access_count: 0,
            },
            relevance_score: 0.85,
            distance: 0.15,
            provenance: provenance.map(|s| s.to_string()),
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

        let prompt = SystemPromptBuilder::build(&config, soul, identity, user, &memories, &[]);

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
            SystemPromptBuilder::build(&config, "Soul content", "Identity content", "User context", &[], &[]);

        assert!(prompt.contains("<soul>"));
        assert!(prompt.contains("<identity>"));
        assert!(prompt.contains("<user_context>"));
        assert!(!prompt.contains("<session_memory>"));
        assert!(!prompt.contains("<long_term_memory>"));
        assert!(prompt.contains("<instructions>"));
    }

    #[test]
    fn test_build_empty_identity_uses_config_fallback() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build(&config, "Soul content", "", "", &[], &[]);

        assert!(prompt.contains("Name: Luna"));
        assert!(prompt.contains("Emoji: 🌙"));
        assert!(prompt.contains("Model: claude-sonnet-4-20250514"));
        assert!(!prompt.contains("<user_context>"));
    }

    #[test]
    fn test_build_empty_soul_omits_section() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build(&config, "", "Identity", "", &[], &[]);

        assert!(!prompt.contains("<soul>"));
        assert!(prompt.contains("<identity>"));
    }

    #[test]
    fn test_memory_format_includes_category() {
        let config = test_config();
        let memories = vec![test_memory("Likes cats", MemoryCategory::Preference)];

        let prompt = SystemPromptBuilder::build(&config, "Soul", "Identity", "", &memories, &[]);

        assert!(prompt.contains("- [preference] Likes cats"));
    }

    #[test]
    fn test_long_term_memory_section() {
        let config = test_config();
        let recalled = vec![
            test_ranked_memory("User loves Rust programming", MemoryCategory::Preference, None),
            test_ranked_memory("User works at Acme Corp", MemoryCategory::Fact, None),
        ];

        let prompt = SystemPromptBuilder::build(&config, "Soul", "Identity", "", &[], &recalled);

        assert!(prompt.contains("<long_term_memory>"));
        assert!(prompt.contains("</long_term_memory>"));
        assert!(prompt.contains("Things you know about the user from past interactions:"));
        assert!(prompt.contains("- User loves Rust programming"));
        assert!(prompt.contains("- User works at Acme Corp"));
        // No scores or metadata visible
        assert!(!prompt.contains("0.85"));
        assert!(!prompt.contains("0.15"));
        assert!(!prompt.contains("relevance"));
    }

    #[test]
    fn test_long_term_memory_with_provenance() {
        let config = test_config();
        let recalled = vec![
            test_ranked_memory("User is a cat lover", MemoryCategory::Preference, Some("Written by BotX")),
        ];

        let prompt = SystemPromptBuilder::build(&config, "Soul", "Identity", "", &[], &recalled);

        assert!(prompt.contains("- User is a cat lover (Written by BotX)"));
    }

    #[test]
    fn test_no_long_term_memory_section_when_empty() {
        let config = test_config();

        let prompt = SystemPromptBuilder::build(&config, "Soul", "Identity", "", &[], &[]);

        assert!(!prompt.contains("<long_term_memory>"));
        assert!(!prompt.contains("</long_term_memory>"));
    }

    #[test]
    fn test_both_session_and_long_term_memories() {
        let config = test_config();
        let session_memories = vec![test_memory("User prefers dark mode", MemoryCategory::Preference)];
        let recalled = vec![test_ranked_memory("User is a Rust developer", MemoryCategory::Fact, None)];

        let prompt = SystemPromptBuilder::build(&config, "Soul", "Identity", "", &session_memories, &recalled);

        assert!(prompt.contains("<session_memory>"));
        assert!(prompt.contains("<long_term_memory>"));
        // Session memory appears before long-term memory
        let session_pos = prompt.find("<session_memory>").unwrap();
        let ltm_pos = prompt.find("<long_term_memory>").unwrap();
        assert!(session_pos < ltm_pos);
    }

    #[test]
    fn test_build_with_capabilities_includes_agent_capabilities() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build_with_capabilities(
            &config,
            "Creative soul",
            "Identity content",
            "User context",
            &[],
            &[],
        );

        assert!(prompt.contains("<agent_capabilities>"));
        assert!(prompt.contains("</agent_capabilities>"));
        assert!(prompt.contains("spawn sub-agents"));
        assert!(prompt.contains("<spawn_agents"));
        // Also has regular sections
        assert!(prompt.contains("<soul>"));
        assert!(prompt.contains("<instructions>"));
    }

    #[test]
    fn test_build_for_sub_agent_includes_soul_but_not_user_context() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build_for_sub_agent(
            &config,
            "I am a creative assistant.",
            "Name: Luna",
            "Research quantum computing",
            1,
        );

        assert!(prompt.contains("<soul>"));
        assert!(prompt.contains("I am a creative assistant."));
        assert!(prompt.contains("<identity>"));
        assert!(prompt.contains("<task>"));
        assert!(prompt.contains("Research quantum computing"));
        assert!(prompt.contains("<sub_agent_instructions>"));
        // Does NOT include user_context, session_memory, long_term_memory
        assert!(!prompt.contains("<user_context>"));
        assert!(!prompt.contains("<session_memory>"));
        assert!(!prompt.contains("<long_term_memory>"));
    }

    #[test]
    fn test_build_for_sub_agent_depth_less_than_3_includes_capabilities() {
        let config = test_config();

        for depth in [0, 1, 2] {
            let prompt = SystemPromptBuilder::build_for_sub_agent(
                &config,
                "Soul",
                "Identity",
                "Some task",
                depth,
            );
            assert!(
                prompt.contains("<agent_capabilities>"),
                "depth {depth} should include agent_capabilities"
            );
        }
    }

    #[test]
    fn test_build_for_sub_agent_depth_3_excludes_capabilities() {
        let config = test_config();
        let prompt = SystemPromptBuilder::build_for_sub_agent(
            &config,
            "Soul",
            "Identity",
            "Some task",
            3,
        );

        assert!(!prompt.contains("<agent_capabilities>"));
    }

    #[test]
    fn test_build_with_skills_includes_available_skills() {
        use boternity_types::skill::SkillManifest;
        use std::path::PathBuf;

        let config = test_config();
        let all_skills = vec![(
            SkillManifest {
                name: "web-search".to_string(),
                description: "Search the web".to_string(),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
            PathBuf::from("/skills/web-search"),
        )];

        let active_skills = vec![(
            SkillManifest {
                name: "greeter".to_string(),
                description: "Greet users".to_string(),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
            "Always say hello!".to_string(),
        )];

        let prompt = SystemPromptBuilder::build_with_skills(
            &config,
            "Soul content",
            "Identity content",
            "User context",
            &[],
            &[],
            &all_skills,
            &active_skills,
        );

        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("name=\"web-search\""));
        assert!(prompt.contains("<active_skills>"));
        assert!(prompt.contains("name=\"greeter\""));
        assert!(prompt.contains("Always say hello!"));
        // Base sections still present
        assert!(prompt.contains("<soul>"));
        assert!(prompt.contains("<instructions>"));
    }

    #[test]
    fn test_build_with_skills_no_skills_equals_build() {
        let config = test_config();
        let soul = "I am a creative assistant.";
        let identity = "Name: Luna";
        let user = "Be concise.";
        let memories = vec![test_memory("Likes cats", MemoryCategory::Preference)];
        let recalled = vec![test_ranked_memory("Knows Rust", MemoryCategory::Fact, None)];

        let base = SystemPromptBuilder::build(&config, soul, identity, user, &memories, &recalled);
        let with_skills = SystemPromptBuilder::build_with_skills(
            &config,
            soul,
            identity,
            user,
            &memories,
            &recalled,
            &[],
            &[],
        );

        assert_eq!(base, with_skills);
    }
}
