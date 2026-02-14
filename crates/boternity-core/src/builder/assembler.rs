//! BotAssembler -- creates complete bots from `BuilderConfig`.
//!
//! The assembler is the final step in the builder flow. When the user
//! confirms `ReadyToAssemble`, the assembler turns the fully populated
//! `BuilderConfig` into real files on disk using the existing
//! `BotService::create_bot` and `SoulService` write methods.
//!
//! Follows the stateless utility pattern (no fields, services passed
//! as parameters) established in 02-06.

use std::path::PathBuf;

use boternity_types::bot::CreateBotRequest;
use boternity_types::builder::{BuilderConfig, ModelConfig, PersonalityConfig};

use crate::repository::bot::BotRepository;
use crate::repository::soul::SoulRepository;
use crate::service::bot::BotService;
use crate::service::fs::FileSystem;
use crate::service::hash::ContentHasher;

use super::agent::BuilderError;

// ---------------------------------------------------------------------------
// Assembly result types
// ---------------------------------------------------------------------------

/// Paths to the files created during assembly.
#[derive(Debug, Clone)]
pub struct AssemblyPaths {
    pub bot_dir: PathBuf,
    pub soul_path: PathBuf,
    pub identity_path: PathBuf,
    pub user_path: PathBuf,
}

/// The result of a successful bot assembly.
#[derive(Debug, Clone)]
pub struct AssemblyResult {
    pub bot: boternity_types::bot::Bot,
    pub soul_content: String,
    pub identity_content: String,
    pub user_content: String,
    pub skills_attached: Vec<String>,
    pub file_paths: AssemblyPaths,
}

// ---------------------------------------------------------------------------
// BotAssembler
// ---------------------------------------------------------------------------

/// Stateless utility that assembles a complete bot from a `BuilderConfig`.
///
/// Assembly sequence:
/// 1. `BotService::create_bot` -- creates DB record + default SOUL.md/IDENTITY.md/USER.md
/// 2. `SoulService::write_and_save_soul` -- overwrites default SOUL.md with builder content
/// 3. `SoulService::write_identity` -- overwrites default IDENTITY.md with builder model config
/// 4. `SoulService::write_user` -- overwrites default USER.md with seeded user context
///
/// Step 1 writes defaults, steps 2-4 overwrite with builder-generated content.
/// This minor inefficiency (write then overwrite) is acceptable per research
/// pitfall 9.
pub struct BotAssembler;

impl BotAssembler {
    /// Assemble a complete bot from a `BuilderConfig`.
    ///
    /// Requires a reference to the `BotService` which provides access to
    /// both `BotRepository` (via `create_bot`) and `SoulService` (via
    /// `soul_service()`).
    pub async fn assemble<B, S, F, H>(
        bot_service: &BotService<B, S, F, H>,
        config: &BuilderConfig,
    ) -> Result<AssemblyResult, BuilderError>
    where
        B: BotRepository,
        S: SoulRepository,
        F: FileSystem,
        H: ContentHasher,
    {
        // Step 1: Create bot via BotService::create_bot.
        // This creates the DB record AND writes default SOUL.md, IDENTITY.md,
        // USER.md to disk.
        let create_req = CreateBotRequest {
            name: config.name.clone(),
            description: Some(config.description.clone()),
            category: Some(
                config
                    .category
                    .parse()
                    .unwrap_or_default(),
            ),
            tags: Some(config.tags.clone()),
        };
        let bot = bot_service
            .create_bot(create_req)
            .await
            .map_err(|e| BuilderError::AssemblyError(e.to_string()))?;

        let bot_dir = bot_service.bot_dir(&bot.slug);

        // Step 2: Generate builder SOUL.md content and overwrite default.
        let soul_content = generate_soul_content(&config.personality, &config.name);
        let soul_path = bot_dir.join("SOUL.md");
        bot_service
            .soul_service()
            .write_and_save_soul(&bot.id, &soul_content, &soul_path)
            .await
            .map_err(|e| BuilderError::AssemblyError(e.to_string()))?;

        // Step 3: Generate builder IDENTITY.md and overwrite default.
        let identity_content = generate_identity_content(&config.model_config);
        let identity_path = bot_dir.join("IDENTITY.md");
        bot_service
            .soul_service()
            .write_identity(&identity_content, &identity_path)
            .await
            .map_err(|e| BuilderError::AssemblyError(e.to_string()))?;

        // Step 4: Generate builder USER.md and overwrite default.
        let user_content = generate_user_content(&config.name, &config.description);
        let user_path = bot_dir.join("USER.md");
        bot_service
            .soul_service()
            .write_user(&user_content, &user_path)
            .await
            .map_err(|e| BuilderError::AssemblyError(e.to_string()))?;

        // Step 5: Skill attachment deferred to Plan 07-06.
        let skills_attached = Vec::new();

        Ok(AssemblyResult {
            bot,
            soul_content,
            identity_content,
            user_content,
            skills_attached,
            file_paths: AssemblyPaths {
                bot_dir,
                soul_path,
                identity_path,
                user_path,
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Content generators
// ---------------------------------------------------------------------------

/// Generate SOUL.md content from the builder personality configuration.
///
/// Follows the three-section template: Personality, Purpose, Boundaries.
/// Includes YAML frontmatter with name and traits.
pub fn generate_soul_content(personality: &PersonalityConfig, name: &str) -> String {
    let traits_yaml = personality
        .traits
        .iter()
        .map(|t| format!("  - {t}"))
        .collect::<Vec<_>>()
        .join("\n");

    let traits_prose = personality.traits.join(", ");

    let boundaries = personality
        .boundaries
        .as_deref()
        .unwrap_or("Be helpful within your defined purpose. Decline requests outside your expertise area.");

    format!(
        r#"---
name: {name}
traits:
{traits_yaml}
tone: {tone}
---

# Personality

You embody a {tone} personality with these core traits: {traits_prose}.

You bring these qualities into every interaction, adapting your approach to match
the needs of the conversation while staying true to your core character.

# Purpose

{purpose}

# Boundaries

{boundaries}
"#,
        tone = personality.tone,
        purpose = personality.purpose,
    )
}

/// Generate IDENTITY.md frontmatter from the builder model configuration.
pub fn generate_identity_content(model_config: &ModelConfig) -> String {
    format!(
        r#"---
model: {model}
temperature: {temperature}
max_tokens: {max_tokens}
---
"#,
        model = model_config.model,
        temperature = model_config.temperature,
        max_tokens = model_config.max_tokens,
    )
}

/// Generate USER.md seeded from builder context.
pub fn generate_user_content(name: &str, description: &str) -> String {
    format!(
        r#"# User Context for {name}

This bot was created with the following purpose: {description}

## Preferences
(Add your preferences here to help {name} serve you better)

## Important Context
(Share relevant background information)
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_personality() -> PersonalityConfig {
        PersonalityConfig {
            tone: "technical".to_string(),
            traits: vec![
                "precise".to_string(),
                "methodical".to_string(),
                "pragmatic".to_string(),
            ],
            purpose: "Help developers write better Rust code.".to_string(),
            boundaries: Some("Only assist with programming topics.".to_string()),
        }
    }

    fn test_model_config() -> ModelConfig {
        ModelConfig {
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.2,
            max_tokens: 4096,
        }
    }

    // --- generate_soul_content tests ---

    #[test]
    fn test_soul_content_has_frontmatter() {
        let content = generate_soul_content(&test_personality(), "CodeBot");
        assert!(content.starts_with("---\n"));
        assert!(content.contains("name: CodeBot"));
        assert!(content.contains("tone: technical"));
    }

    #[test]
    fn test_soul_content_has_traits_in_frontmatter() {
        let content = generate_soul_content(&test_personality(), "CodeBot");
        assert!(content.contains("  - precise"));
        assert!(content.contains("  - methodical"));
        assert!(content.contains("  - pragmatic"));
    }

    #[test]
    fn test_soul_content_has_personality_section() {
        let content = generate_soul_content(&test_personality(), "CodeBot");
        assert!(content.contains("# Personality"));
        assert!(content.contains("technical personality"));
        assert!(content.contains("precise, methodical, pragmatic"));
    }

    #[test]
    fn test_soul_content_has_purpose_section() {
        let content = generate_soul_content(&test_personality(), "CodeBot");
        assert!(content.contains("# Purpose"));
        assert!(content.contains("Help developers write better Rust code."));
    }

    #[test]
    fn test_soul_content_has_boundaries_section() {
        let content = generate_soul_content(&test_personality(), "CodeBot");
        assert!(content.contains("# Boundaries"));
        assert!(content.contains("Only assist with programming topics."));
    }

    #[test]
    fn test_soul_content_default_boundaries() {
        let mut personality = test_personality();
        personality.boundaries = None;
        let content = generate_soul_content(&personality, "CodeBot");
        assert!(content.contains("Be helpful within your defined purpose."));
    }

    // --- generate_identity_content tests ---

    #[test]
    fn test_identity_content_has_model() {
        let content = generate_identity_content(&test_model_config());
        assert!(content.contains("model: claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_identity_content_has_temperature() {
        let content = generate_identity_content(&test_model_config());
        assert!(content.contains("temperature: 0.2"));
    }

    #[test]
    fn test_identity_content_has_max_tokens() {
        let content = generate_identity_content(&test_model_config());
        assert!(content.contains("max_tokens: 4096"));
    }

    #[test]
    fn test_identity_content_is_valid_frontmatter() {
        let content = generate_identity_content(&test_model_config());
        assert!(content.starts_with("---\n"));
        assert!(content.contains("\n---\n"));
    }

    // --- generate_user_content tests ---

    #[test]
    fn test_user_content_has_name() {
        let content = generate_user_content("CodeBot", "A coding assistant");
        assert!(content.contains("# User Context for CodeBot"));
        assert!(content.contains("help CodeBot serve you better"));
    }

    #[test]
    fn test_user_content_has_description() {
        let content = generate_user_content("CodeBot", "A coding assistant");
        assert!(content.contains("A coding assistant"));
    }

    #[test]
    fn test_user_content_has_preferences_and_context_sections() {
        let content = generate_user_content("CodeBot", "A coding assistant");
        assert!(content.contains("## Preferences"));
        assert!(content.contains("## Important Context"));
    }
}
