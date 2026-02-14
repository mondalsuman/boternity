//! Forge system prompt builder with memory recall.
//!
//! Constructs the system prompt for Forge (the builder LLM) using XML tag
//! boundaries consistent with the project pattern (02-05 decision). The
//! prompt includes accumulated state context, mode-specific instructions,
//! and recalled builder memories for cross-session suggestion continuity.

use boternity_types::builder::{BuilderPhase, BuilderState};

use crate::builder::state::BuilderStateExt;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A lightweight view of a past builder session for prompt injection.
///
/// The caller (LlmBuilderAgent in Plan 07-05) queries the builder memory
/// store and passes these to the prompt builder. This enables Forge to
/// reference past choices: "Last time you made a coding bot, you chose
/// formal tone -- same here?"
#[derive(Debug, Clone)]
pub struct RecalledBuilderMemory {
    pub initial_description: String,
    pub chosen_tone: Option<String>,
    pub chosen_model: Option<String>,
    pub chosen_skills: Vec<String>,
    pub bot_slug: Option<String>,
}

/// Mode of the builder session, controlling instruction variants.
#[derive(Debug, Clone)]
pub enum BuilderMode {
    /// Creating a new bot from scratch.
    NewBot,
    /// Reconfiguring an existing bot.
    ReconfigureBot,
    /// Creating a new skill.
    NewSkill,
    /// Batch-creating multiple bot variants.
    BatchCreate { variant_count: usize },
}

// ---------------------------------------------------------------------------
// Forge identity
// ---------------------------------------------------------------------------

/// Forge's SOUL content as an embedded constant.
///
/// Forge is NOT a real bot in the database -- it is a built-in personality
/// embedded as a const (per research pitfall 6). This avoids requiring a
/// database entry for the builder agent.
pub fn build_forge_soul_content() -> &'static str {
    r#"# Forge -- The Bot Builder

## Personality
You are Forge, the bot builder. You're a helpful craftsman -- warm, encouraging,
slightly casual, like a skilled teammate walking someone through their first build.
You take genuine pride in helping users create bots that feel right.

## Purpose
Guide users through bot and skill creation via an interactive conversation. Ask
smart questions, suggest good defaults, explain your reasoning, and ensure the
final configuration truly matches what the user envisioned.

## Boundaries
- Never create a bot without explicit user confirmation at the review stage
- Always explain why you're suggesting a particular option
- If the user seems unsure, offer concrete examples rather than abstract choices
- Respect the user's expertise level -- don't over-explain to advanced users
- Keep the conversation moving forward; don't ask unnecessary questions"#
}

// ---------------------------------------------------------------------------
// Phase labels
// ---------------------------------------------------------------------------

/// Human-readable label for a builder phase.
///
/// These labels are shown to the user as progress indicators during the
/// builder wizard flow.
pub fn format_phase_label(phase: &BuilderPhase) -> &'static str {
    match phase {
        BuilderPhase::Basics => "Setting up basics...",
        BuilderPhase::Personality => "Defining personality...",
        BuilderPhase::Model => "Choosing model...",
        BuilderPhase::Skills => "Selecting skills...",
        BuilderPhase::Review => "Final review...",
    }
}

// ---------------------------------------------------------------------------
// System prompt builder
// ---------------------------------------------------------------------------

/// Build the complete Forge system prompt with XML-tagged sections.
///
/// The prompt includes:
/// - `<forge_identity>`: Forge's persona and role
/// - `<builder_instructions>`: Mode-specific rules for the conversation
/// - `<accumulated_context>`: State from prior exchanges in this session
/// - `<current_config>`: Non-None fields from the partial config
/// - `<past_sessions>`: Recalled builder memories (omitted when empty)
///
/// The `recalled_memories` parameter enables cross-session continuity:
/// Forge can reference past choices to suggest similar options for the
/// current session.
pub fn build_forge_system_prompt(
    state: &BuilderState,
    mode: &BuilderMode,
    recalled_memories: &[RecalledBuilderMemory],
) -> String {
    let mut sections = Vec::with_capacity(6);

    // Forge identity section
    sections.push(format!(
        "<forge_identity>\n{}\n</forge_identity>",
        build_forge_soul_content()
    ));

    // Builder instructions section (mode-specific)
    sections.push(format!(
        "<builder_instructions>\n{}\n</builder_instructions>",
        build_instructions(mode)
    ));

    // Accumulated context section
    sections.push(format!(
        "<accumulated_context>\n{}\n</accumulated_context>",
        build_accumulated_context(state)
    ));

    // Current config section (only non-None fields)
    let config_summary = build_config_summary(&state.config);
    if !config_summary.is_empty() {
        sections.push(format!(
            "<current_config>\n{}\n</current_config>",
            config_summary
        ));
    }

    // Past sessions section (only when recalled_memories is non-empty)
    if !recalled_memories.is_empty() {
        sections.push(format!(
            "<past_sessions>\n{}\n</past_sessions>",
            build_past_sessions(recalled_memories)
        ));
    }

    sections.join("\n\n")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn build_instructions(mode: &BuilderMode) -> String {
    let mode_label = match mode {
        BuilderMode::NewBot => "new bot creation",
        BuilderMode::ReconfigureBot => "bot reconfiguration",
        BuilderMode::NewSkill => "new skill creation",
        BuilderMode::BatchCreate { .. } => "batch bot creation",
    };

    let mode_specific = match mode {
        BuilderMode::ReconfigureBot => {
            "\n8. Show the current configuration first and ask \"What would you like to adjust?\""
        }
        BuilderMode::BatchCreate { variant_count } => {
            // We need to return a string that includes the variant count, so use a different approach
            return format!(
                "You are conducting a {mode_label} session. Generate your response as structured JSON matching the BuilderTurn schema.\n\
                \n\
                RULES:\n\
                1. Ask ONE question at a time with multi-choice options\n\
                2. Every option MUST have a brief description explaining when it's best suited\n\
                3. Always include an \"Other\" option for free-text input\n\
                4. Adapt question depth to the purpose complexity:\n\
                   - Simple utility bots: 3-5 questions, smart defaults\n\
                   - Complex analyst/research bots: 6-10 questions, probe for details\n\
                   - Creative bots: 4-7 questions, focus on personality\n\
                5. Show phase labels (\"Setting up basics...\", \"Defining personality...\", etc.)\n\
                6. Always explain your reasoning for suggestions\n\
                7. When you have enough context, signal ready_to_assemble with the full config\n\
                8. Ask \"What makes this variant different?\" for each of the {variant_count} variants"
            );
        }
        BuilderMode::NewSkill => {
            "\n8. Ask about skill name, description, type (local/wasm), trigger conditions, and required capabilities. When done, signal ready_to_assemble with the skill config."
        }
        BuilderMode::NewBot => "",
    };

    format!(
        "You are conducting a {mode_label} session. Generate your response as structured JSON matching the BuilderTurn schema.\n\
        \n\
        RULES:\n\
        1. Ask ONE question at a time with multi-choice options\n\
        2. Every option MUST have a brief description explaining when it's best suited\n\
        3. Always include an \"Other\" option for free-text input\n\
        4. Adapt question depth to the purpose complexity:\n\
           - Simple utility bots: 3-5 questions, smart defaults\n\
           - Complex analyst/research bots: 6-10 questions, probe for details\n\
           - Creative bots: 4-7 questions, focus on personality\n\
        5. Show phase labels (\"Setting up basics...\", \"Defining personality...\", etc.)\n\
        6. Always explain your reasoning for suggestions\n\
        7. When you have enough context, signal ready_to_assemble with the full config{mode_specific}"
    )
}

fn build_accumulated_context(state: &BuilderState) -> String {
    let purpose = state
        .purpose_category
        .as_ref()
        .map(|p| format!("{p:?}"))
        .unwrap_or_else(|| "not yet determined".to_string());

    format!(
        "Initial description: {}\n\
        Current phase: {}\n\
        Purpose category: {}\n\
        Questions asked: {}\n\
        \n\
        Previous exchanges:\n\
        {}",
        state.initial_description,
        format_phase_label(&state.phase),
        purpose,
        state.question_count(),
        state.conversation_summary(),
    )
}

fn build_config_summary(config: &boternity_types::builder::PartialBuilderConfig) -> String {
    let mut lines = Vec::new();

    if let Some(name) = &config.name {
        lines.push(format!("Name: {name}"));
    }
    if let Some(desc) = &config.description {
        lines.push(format!("Description: {desc}"));
    }
    if let Some(cat) = &config.category {
        lines.push(format!("Category: {cat}"));
    }
    if let Some(tags) = &config.tags {
        lines.push(format!("Tags: {}", tags.join(", ")));
    }
    if let Some(tone) = &config.tone {
        lines.push(format!("Tone: {tone}"));
    }
    if let Some(traits) = &config.traits {
        lines.push(format!("Traits: {}", traits.join(", ")));
    }
    if let Some(purpose) = &config.purpose {
        lines.push(format!("Purpose: {purpose}"));
    }
    if let Some(boundaries) = &config.boundaries {
        lines.push(format!("Boundaries: {boundaries}"));
    }
    if let Some(model) = &config.model {
        lines.push(format!("Model: {model}"));
    }
    if let Some(temp) = &config.temperature {
        lines.push(format!("Temperature: {temp}"));
    }
    if let Some(max) = &config.max_tokens {
        lines.push(format!("Max tokens: {max}"));
    }
    if !config.skills.is_empty() {
        let skill_names: Vec<&str> = config.skills.iter().map(|s| s.name.as_str()).collect();
        lines.push(format!("Skills: {}", skill_names.join(", ")));
    }

    lines.join("\n")
}

fn build_past_sessions(memories: &[RecalledBuilderMemory]) -> String {
    let mut lines = Vec::new();
    lines.push("The user has created bots before. Here are relevant past choices you can reference to suggest similar options:".to_string());

    for memory in memories {
        let slug = memory
            .bot_slug
            .as_deref()
            .unwrap_or("a bot");
        let tone = memory
            .chosen_tone
            .as_deref()
            .unwrap_or("not set");
        let model = memory
            .chosen_model
            .as_deref()
            .unwrap_or("default");
        let skills = if memory.chosen_skills.is_empty() {
            "none".to_string()
        } else {
            memory.chosen_skills.join(", ")
        };

        lines.push(format!(
            "- Previously created \"{slug}\": description \"{}\", tone: {tone}, model: {model}, skills: [{skills}]",
            memory.initial_description
        ));
    }

    lines.push(
        "When relevant, mention these past choices: \"Last time you made a similar bot, you chose X -- would you like the same here?\"".to_string()
    );

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::state::new_builder_state;
    use uuid::Uuid;

    fn test_state() -> BuilderState {
        new_builder_state(Uuid::now_v7(), "A helpful coding assistant".to_string())
    }

    #[test]
    fn test_build_forge_system_prompt_contains_xml_tags() {
        let state = test_state();
        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewBot, &[]);

        assert!(prompt.contains("<forge_identity>"));
        assert!(prompt.contains("</forge_identity>"));
        assert!(prompt.contains("<builder_instructions>"));
        assert!(prompt.contains("</builder_instructions>"));
        assert!(prompt.contains("<accumulated_context>"));
        assert!(prompt.contains("</accumulated_context>"));
    }

    #[test]
    fn test_build_forge_system_prompt_includes_accumulated_context() {
        let mut state = test_state();
        state.record_exchange("What is your bot's name?".to_string(), "CodeHelper".to_string());

        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewBot, &[]);

        assert!(prompt.contains("Initial description: A helpful coding assistant"));
        assert!(prompt.contains("Questions asked: 1"));
        assert!(prompt.contains("What is your bot's name?"));
        assert!(prompt.contains("CodeHelper"));
    }

    #[test]
    fn test_with_recalled_memories_includes_past_sessions() {
        let state = test_state();
        let memories = vec![RecalledBuilderMemory {
            initial_description: "A writing assistant".to_string(),
            chosen_tone: Some("formal".to_string()),
            chosen_model: Some("claude-sonnet-4-20250514".to_string()),
            chosen_skills: vec!["grammar-check".to_string()],
            bot_slug: Some("writer-bot".to_string()),
        }];

        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewBot, &memories);

        assert!(prompt.contains("<past_sessions>"));
        assert!(prompt.contains("</past_sessions>"));
        assert!(prompt.contains("writer-bot"));
        assert!(prompt.contains("formal"));
        assert!(prompt.contains("grammar-check"));
        assert!(prompt.contains("Last time you made a similar bot"));
    }

    #[test]
    fn test_empty_recalled_memories_omits_past_sessions() {
        let state = test_state();
        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewBot, &[]);

        assert!(!prompt.contains("<past_sessions>"));
        assert!(!prompt.contains("</past_sessions>"));
    }

    #[test]
    fn test_format_phase_label_returns_correct_labels() {
        assert_eq!(
            format_phase_label(&BuilderPhase::Basics),
            "Setting up basics..."
        );
        assert_eq!(
            format_phase_label(&BuilderPhase::Personality),
            "Defining personality..."
        );
        assert_eq!(
            format_phase_label(&BuilderPhase::Model),
            "Choosing model..."
        );
        assert_eq!(
            format_phase_label(&BuilderPhase::Skills),
            "Selecting skills..."
        );
        assert_eq!(
            format_phase_label(&BuilderPhase::Review),
            "Final review..."
        );
    }

    #[test]
    fn test_build_forge_soul_content_is_non_empty() {
        let content = build_forge_soul_content();
        assert!(!content.is_empty());
        assert!(content.contains("Personality"));
        assert!(content.contains("Purpose"));
        assert!(content.contains("Boundaries"));
        assert!(content.contains("Forge"));
    }

    #[test]
    fn test_reconfigure_mode_includes_specific_instruction() {
        let state = test_state();
        let prompt = build_forge_system_prompt(&state, &BuilderMode::ReconfigureBot, &[]);

        assert!(prompt.contains("bot reconfiguration"));
        assert!(prompt.contains("What would you like to adjust?"));
    }

    #[test]
    fn test_batch_mode_includes_variant_instruction() {
        let state = test_state();
        let prompt = build_forge_system_prompt(
            &state,
            &BuilderMode::BatchCreate { variant_count: 3 },
            &[],
        );

        assert!(prompt.contains("batch bot creation"));
        assert!(prompt.contains("3 variants"));
    }

    #[test]
    fn test_config_summary_only_shows_set_fields() {
        let mut state = test_state();
        state.config.name = Some("Luna".to_string());
        state.config.model = Some("claude-sonnet-4-20250514".to_string());

        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewBot, &[]);

        assert!(prompt.contains("<current_config>"));
        assert!(prompt.contains("Name: Luna"));
        assert!(prompt.contains("Model: claude-sonnet-4-20250514"));
        // Fields not set should not appear
        assert!(!prompt.contains("Temperature:"));
        assert!(!prompt.contains("Boundaries:"));
    }

    #[test]
    fn test_empty_config_omits_current_config_section() {
        let state = test_state();
        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewBot, &[]);

        // Config is all None/empty, so <current_config> should be omitted
        assert!(!prompt.contains("<current_config>"));
    }

    #[test]
    fn test_new_skill_mode_includes_specific_instruction() {
        let state = test_state();
        let prompt = build_forge_system_prompt(&state, &BuilderMode::NewSkill, &[]);

        assert!(prompt.contains("new skill creation"));
        assert!(prompt.contains("skill name, description, type"));
    }
}
