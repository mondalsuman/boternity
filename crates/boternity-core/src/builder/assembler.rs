//! BotAssembler -- creates complete bots from `BuilderConfig`.
//!
//! The assembler is the final step in the builder flow. When the user
//! confirms `ReadyToAssemble`, the assembler turns the fully populated
//! `BuilderConfig` into real files on disk using the existing
//! `BotService::create_bot` and `SoulService` write methods.
//!
//! Follows the stateless utility pattern (no fields, services passed
//! as parameters) established in 02-06.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use boternity_types::bot::CreateBotRequest;
use boternity_types::builder::{BuilderConfig, ModelConfig, PersonalityConfig};
use boternity_types::skill::{BotSkillConfig, BotSkillsFile, TrustTier};

use crate::repository::bot::BotRepository;
use crate::repository::soul::SoulRepository;
use crate::service::bot::BotService;
use crate::service::fs::FileSystem;
use crate::service::hash::ContentHasher;
use crate::skill::manifest::serialize_bot_skills_config;

use super::agent::BuilderError;
use super::skill_builder::SkillBuildResult;

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

        // Step 5: Attach skills if any were requested.
        let skills_attached = if !config.skills.is_empty() {
            // Build SkillBuildResults from SkillRequests (lightweight -- no LLM call here,
            // these are already-generated manifests from the builder flow).
            let skill_results: Vec<SkillBuildResult> = config
                .skills
                .iter()
                .map(|sr| skill_request_to_build_result(sr))
                .collect();
            Self::attach_skills(&bot.slug, &skill_results, &bot_dir)?
        } else {
            Vec::new()
        };

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

    /// Attach skills to a bot by writing SKILL.md files and updating skills.toml.
    ///
    /// For each `SkillBuildResult`:
    /// - Creates `{data_dir}/skills/{skill_name}/SKILL.md`
    /// - If source code is present, writes `{data_dir}/skills/{skill_name}/src/lib.rs`
    /// - Updates `{data_dir}/skills.toml` with the skill configuration
    ///
    /// Skills are tagged with `builder-created` origin metadata.
    /// Returns the list of attached skill names.
    pub fn attach_skills(
        _bot_slug: &str,
        skills: &[SkillBuildResult],
        data_dir: &Path,
    ) -> Result<Vec<String>, BuilderError> {
        let mut attached_names = Vec::new();
        let mut skills_config = HashMap::new();

        for skill in skills {
            let skill_name = &skill.manifest.name;
            let skill_dir = data_dir.join("skills").join(skill_name);

            // Create skill directory
            std::fs::create_dir_all(&skill_dir).map_err(|e| {
                BuilderError::AssemblyError(format!(
                    "Failed to create skill directory {}: {e}",
                    skill_dir.display()
                ))
            })?;

            // Write SKILL.md
            let skill_md_path = skill_dir.join("SKILL.md");
            std::fs::write(&skill_md_path, &skill.skill_md_content).map_err(|e| {
                BuilderError::AssemblyError(format!(
                    "Failed to write {}: {e}",
                    skill_md_path.display()
                ))
            })?;

            // Write source code if present (WASM skills)
            if let Some(ref source) = skill.source_code {
                let src_dir = skill_dir.join("src");
                std::fs::create_dir_all(&src_dir).map_err(|e| {
                    BuilderError::AssemblyError(format!(
                        "Failed to create src directory {}: {e}",
                        src_dir.display()
                    ))
                })?;
                let lib_path = src_dir.join("lib.rs");
                std::fs::write(&lib_path, source).map_err(|e| {
                    BuilderError::AssemblyError(format!(
                        "Failed to write {}: {e}",
                        lib_path.display()
                    ))
                })?;
            }

            // Build per-skill config entry with builder-created tag
            let mut overrides = HashMap::new();
            overrides.insert("origin".to_owned(), "builder-created".to_owned());

            let capabilities = skill
                .manifest
                .metadata
                .as_ref()
                .and_then(|m| m.capabilities.clone());

            skills_config.insert(
                skill_name.clone(),
                BotSkillConfig {
                    skill_name: skill_name.clone(),
                    enabled: true,
                    trust_tier: Some(TrustTier::Local),
                    version: skill
                        .manifest
                        .metadata
                        .as_ref()
                        .and_then(|m| m.version.clone()),
                    overrides,
                    capabilities,
                },
            );

            attached_names.push(skill_name.clone());
        }

        // Write skills.toml
        if !skills_config.is_empty() {
            let config = BotSkillsFile {
                skills: skills_config,
            };
            let toml_content = serialize_bot_skills_config(&config).map_err(|e| {
                BuilderError::AssemblyError(format!("Failed to serialize skills.toml: {e}"))
            })?;
            let config_path = data_dir.join("skills.toml");
            std::fs::write(&config_path, &toml_content).map_err(|e| {
                BuilderError::AssemblyError(format!(
                    "Failed to write {}: {e}",
                    config_path.display()
                ))
            })?;
        }

        Ok(attached_names)
    }

    /// Format a human-readable assembly summary for CLI display.
    ///
    /// Produces a detailed post-create output with bot name, slug, file paths,
    /// attached skills, model configuration, and next steps.
    pub fn format_assembly_summary(result: &AssemblyResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("Bot Created: {}\n", result.bot.name));
        output.push_str(&format!("Slug: {}\n", result.bot.slug));

        if !result.bot.description.is_empty() {
            output.push_str(&format!("Description: {}\n", result.bot.description));
        }

        output.push_str(&format!("Category: {}\n", result.bot.category));

        // Extract model info from identity content (parse frontmatter)
        if let Some(model_line) = result
            .identity_content
            .lines()
            .find(|l| l.starts_with("model:"))
        {
            let model = model_line.trim_start_matches("model:").trim();
            if let Some(temp_line) = result
                .identity_content
                .lines()
                .find(|l| l.starts_with("temperature:"))
            {
                let temp = temp_line.trim_start_matches("temperature:").trim();
                output.push_str(&format!("Model: {model} (temperature: {temp})\n"));
            } else {
                output.push_str(&format!("Model: {model}\n"));
            }
        }

        output.push_str("\nFiles:\n");
        output.push_str(&format!(
            "  SOUL.md:     {}\n",
            result.file_paths.soul_path.display()
        ));
        output.push_str(&format!(
            "  IDENTITY.md: {}\n",
            result.file_paths.identity_path.display()
        ));
        output.push_str(&format!(
            "  USER.md:     {}\n",
            result.file_paths.user_path.display()
        ));

        if !result.skills_attached.is_empty() {
            output.push_str(&format!(
                "\nSkills ({}):\n",
                result.skills_attached.len()
            ));
            for skill in &result.skills_attached {
                output.push_str(&format!("  - {skill}\n"));
            }
        }

        output.push_str(&format!("\nNext: bnity chat {}\n", result.bot.slug));

        output
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `SkillRequest` (from the builder config) into a minimal
/// `SkillBuildResult` suitable for `attach_skills`.
///
/// This creates a simple SKILL.md from the name/description without an LLM call.
/// The LLM-driven generation happens earlier in the builder flow; this is the
/// fallback for skills that were requested but not yet fully generated.
fn skill_request_to_build_result(
    req: &boternity_types::builder::SkillRequest,
) -> SkillBuildResult {
    let skill_type_str = if req.skill_type == "wasm" {
        "tool"
    } else {
        "prompt"
    };

    let skill_md = format!(
        "---\nname: {name}\ndescription: {desc}\nmetadata:\n  version: \"0.1.0\"\n  skill-type: {stype}\n  author: builder\n---\n\n# {name}\n\n{desc}\n",
        name = req.name,
        desc = req.description,
        stype = skill_type_str,
    );

    let manifest = boternity_types::skill::SkillManifest {
        name: req.name.clone(),
        description: req.description.clone(),
        license: None,
        compatibility: None,
        metadata: Some(boternity_types::skill::SkillMetadata {
            author: Some("builder".to_owned()),
            version: Some("0.1.0".to_owned()),
            skill_type: Some(if req.skill_type == "wasm" {
                boternity_types::skill::SkillType::Tool
            } else {
                boternity_types::skill::SkillType::Prompt
            }),
            capabilities: None,
            dependencies: None,
            conflicts_with: None,
            trust_tier: Some(TrustTier::Local),
            parents: None,
            secrets: None,
            categories: None,
        }),
        allowed_tools: None,
    };

    SkillBuildResult {
        manifest,
        skill_md_content: skill_md,
        source_code: None,
        suggested_capabilities: Vec::new(),
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

    // --- format_assembly_summary tests ---

    fn test_assembly_result() -> AssemblyResult {
        use boternity_types::bot::{Bot, BotCategory, BotId, BotStatus};

        let bot = Bot {
            id: BotId::from_uuid(uuid::Uuid::nil()),
            name: "CodeBot".to_string(),
            slug: "codebot".to_string(),
            description: "A coding assistant".to_string(),
            category: BotCategory::Utility,
            status: BotStatus::Active,
            tags: vec!["rust".to_string()],
            user_id: None,
            conversation_count: 0,
            total_tokens_used: 0,
            version_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_active_at: None,
        };

        AssemblyResult {
            bot,
            soul_content: "soul content".to_string(),
            identity_content: "---\nmodel: claude-sonnet-4-20250514\ntemperature: 0.2\nmax_tokens: 4096\n---\n".to_string(),
            user_content: "user content".to_string(),
            skills_attached: vec!["web-search".to_string(), "code-review".to_string()],
            file_paths: AssemblyPaths {
                bot_dir: PathBuf::from("/data/bots/codebot"),
                soul_path: PathBuf::from("/data/bots/codebot/SOUL.md"),
                identity_path: PathBuf::from("/data/bots/codebot/IDENTITY.md"),
                user_path: PathBuf::from("/data/bots/codebot/USER.md"),
            },
        }
    }

    #[test]
    fn test_format_assembly_summary_includes_all_fields() {
        let result = test_assembly_result();
        let summary = BotAssembler::format_assembly_summary(&result);

        assert!(summary.contains("Bot Created: CodeBot"), "Missing bot name");
        assert!(summary.contains("Slug: codebot"), "Missing slug");
        assert!(summary.contains("Category: utility"), "Missing category");
        assert!(
            summary.contains("Model: claude-sonnet-4-20250514 (temperature: 0.2)"),
            "Missing model info, got:\n{summary}"
        );
        assert!(summary.contains("SOUL.md:"), "Missing SOUL.md path");
        assert!(summary.contains("IDENTITY.md:"), "Missing IDENTITY.md path");
        assert!(summary.contains("USER.md:"), "Missing USER.md path");
        assert!(summary.contains("Skills (2):"), "Missing skills count");
        assert!(summary.contains("- web-search"), "Missing web-search skill");
        assert!(summary.contains("- code-review"), "Missing code-review skill");
        assert!(
            summary.contains("Next: bnity chat codebot"),
            "Missing next step"
        );
    }

    #[test]
    fn test_format_assembly_summary_no_skills() {
        let mut result = test_assembly_result();
        result.skills_attached = Vec::new();
        let summary = BotAssembler::format_assembly_summary(&result);

        assert!(!summary.contains("Skills ("), "Should not show skills section");
        assert!(summary.contains("Next: bnity chat"), "Should have next step");
    }

    // --- attach_skills tests ---

    #[test]
    fn test_attach_skills_writes_files() {
        use super::super::skill_builder::{SkillBuildResult, SuggestedCapability};

        let tmpdir = tempfile::tempdir().unwrap();
        let data_dir = tmpdir.path();

        let skill = SkillBuildResult {
            manifest: boternity_types::skill::SkillManifest {
                name: "test-skill".to_owned(),
                description: "A test skill".to_owned(),
                license: None,
                compatibility: None,
                metadata: Some(boternity_types::skill::SkillMetadata {
                    author: Some("builder".to_owned()),
                    version: Some("0.1.0".to_owned()),
                    skill_type: Some(boternity_types::skill::SkillType::Prompt),
                    capabilities: None,
                    dependencies: None,
                    conflicts_with: None,
                    trust_tier: Some(TrustTier::Local),
                    parents: None,
                    secrets: None,
                    categories: None,
                }),
                allowed_tools: None,
            },
            skill_md_content: "---\nname: test-skill\ndescription: A test skill\n---\n\nTest instructions.".to_owned(),
            source_code: None,
            suggested_capabilities: vec![SuggestedCapability {
                capability: "http_get".to_owned(),
                reason: "Needs network".to_owned(),
            }],
        };

        let attached = BotAssembler::attach_skills("test-bot", &[skill], data_dir).unwrap();

        assert_eq!(attached, vec!["test-skill"]);
        assert!(data_dir.join("skills/test-skill/SKILL.md").exists());
        assert!(data_dir.join("skills.toml").exists());

        // Verify skills.toml content
        let toml_content = std::fs::read_to_string(data_dir.join("skills.toml")).unwrap();
        assert!(toml_content.contains("test-skill"));
        assert!(toml_content.contains("builder-created"));
    }

    #[test]
    fn test_attach_skills_with_source_code() {
        use super::super::skill_builder::SkillBuildResult;

        let tmpdir = tempfile::tempdir().unwrap();
        let data_dir = tmpdir.path();

        let skill = SkillBuildResult {
            manifest: boternity_types::skill::SkillManifest {
                name: "wasm-skill".to_owned(),
                description: "A WASM skill".to_owned(),
                license: None,
                compatibility: None,
                metadata: Some(boternity_types::skill::SkillMetadata {
                    author: Some("builder".to_owned()),
                    version: Some("0.1.0".to_owned()),
                    skill_type: Some(boternity_types::skill::SkillType::Tool),
                    capabilities: None,
                    dependencies: None,
                    conflicts_with: None,
                    trust_tier: Some(TrustTier::Local),
                    parents: None,
                    secrets: None,
                    categories: None,
                }),
                allowed_tools: None,
            },
            skill_md_content: "---\nname: wasm-skill\ndescription: A WASM skill\n---\n\nWASM instructions.".to_owned(),
            source_code: Some("fn main() { /* wasm code */ }".to_owned()),
            suggested_capabilities: Vec::new(),
        };

        let attached = BotAssembler::attach_skills("test-bot", &[skill], data_dir).unwrap();

        assert_eq!(attached, vec!["wasm-skill"]);
        assert!(data_dir.join("skills/wasm-skill/SKILL.md").exists());
        assert!(data_dir.join("skills/wasm-skill/src/lib.rs").exists());

        let source = std::fs::read_to_string(data_dir.join("skills/wasm-skill/src/lib.rs")).unwrap();
        assert!(source.contains("wasm code"));
    }

    #[test]
    fn test_attach_skills_empty_returns_empty() {
        let tmpdir = tempfile::tempdir().unwrap();
        let attached = BotAssembler::attach_skills("test-bot", &[], tmpdir.path()).unwrap();
        assert!(attached.is_empty());
        assert!(!tmpdir.path().join("skills.toml").exists());
    }
}
