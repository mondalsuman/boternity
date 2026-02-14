//! CLI builder wizard for interactive bot creation (`bnity build`).
//!
//! Drives the multi-turn conversation between the user and Forge (the builder
//! agent) using dialoguer for arrow-key selection, back navigation, and live
//! preview. Supports three modes: new, resume (from draft), and reconfigure
//! (existing bot).

use anyhow::{Context, Result};
use console::style;
use dialoguer::{Confirm, Input, Select};
use uuid::Uuid;

use boternity_core::builder::agent::BuilderAgent;
use boternity_core::builder::assembler::BotAssembler;
use boternity_core::builder::draft_store::{BuilderDraft, BuilderDraftStore};
use boternity_core::builder::memory::{BuilderMemoryEntry, BuilderMemoryStore};
use boternity_core::builder::state::{new_builder_state, BuilderStateExt};
use boternity_infra::builder::llm_builder::LlmBuilderAgent;
use boternity_infra::builder::sqlite_memory_store::SqliteBuilderMemoryStore;
use boternity_types::builder::{BuilderAnswer, BuilderState, BuilderTurn};

use crate::state::AppState;

/// Run the interactive builder wizard to create a new bot.
///
/// This is the main entry point for `bnity build`. It creates an
/// `LlmBuilderAgent`, asks for an initial description, and enters the
/// conversation loop.
pub async fn run_builder_wizard(state: &AppState) -> Result<()> {
    let description: String = Input::new()
        .with_prompt("What kind of bot do you want to create?")
        .interact_text()?;

    let session_id = Uuid::now_v7();

    let provider = state
        .create_single_provider("claude-sonnet-4-20250514")
        .await
        .context("Failed to create LLM provider for builder")?;

    let builder = LlmBuilderAgent::new(
        provider,
        Some((*state.builder_memory_store).clone()),
        "claude-sonnet-4-20250514".to_string(),
    );

    let mut builder_state = new_builder_state(session_id, description.clone());

    println!();
    println!(
        "  {} Starting bot builder...",
        style("*").cyan().bold()
    );

    let turn = builder
        .start(session_id, &description)
        .await
        .context("Builder failed to start")?;

    run_conversation_loop(state, &builder, &mut builder_state, turn).await
}

/// Resume a builder session from a saved draft.
///
/// Lists available drafts, lets the user select one, and resumes the
/// conversation from where they left off.
pub async fn run_builder_resume(state: &AppState) -> Result<()> {
    let drafts = state
        .builder_draft_store
        .list_drafts()
        .await
        .context("Failed to list builder drafts")?;

    if drafts.is_empty() {
        println!();
        println!("  No saved drafts found. Start a new session with: bnity build");
        println!();
        return Ok(());
    }

    let items: Vec<String> = drafts
        .iter()
        .map(|d| {
            format!(
                "{} -- {} phase, updated {}",
                d.initial_description,
                d.phase,
                d.updated_at.format("%Y-%m-%d %H:%M")
            )
        })
        .collect();

    println!();
    println!("  {} Saved builder sessions:", style("*").cyan().bold());
    println!();

    let selection = Select::new()
        .items(&items)
        .default(0)
        .interact()?;

    let draft = state
        .builder_draft_store
        .load_draft(&drafts[selection].session_id)
        .await
        .context("Failed to load draft")?
        .context("Draft not found")?;

    let builder_state: BuilderState =
        serde_json::from_str(&draft.state_json).context("Failed to deserialize builder state")?;

    let provider = state
        .create_single_provider("claude-sonnet-4-20250514")
        .await
        .context("Failed to create LLM provider for builder")?;

    let builder = LlmBuilderAgent::new(
        provider,
        Some((*state.builder_memory_store).clone()),
        "claude-sonnet-4-20250514".to_string(),
    );

    println!();
    println!(
        "  {} Resuming builder session: {}",
        style("*").cyan().bold(),
        style(&builder_state.initial_description).yellow()
    );

    let turn = builder
        .resume(&builder_state)
        .await
        .context("Builder failed to resume")?;

    let mut builder_state = builder_state;
    run_conversation_loop(state, &builder, &mut builder_state, turn).await
}

/// Reconfigure an existing bot through the builder wizard.
///
/// Loads the bot's current configuration and enters the builder conversation
/// with the existing config pre-populated.
pub async fn run_builder_reconfigure(state: &AppState, slug: &str) -> Result<()> {
    use boternity_types::builder::{BuilderConfig, ModelConfig, PersonalityConfig};

    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .context("Failed to find bot")?;

    let bot_dir = state.data_dir.join("bots").join(&bot.slug);

    // Read IDENTITY.md for model config
    let identity_path = bot_dir.join("IDENTITY.md");
    let identity_content = tokio::fs::read_to_string(&identity_path)
        .await
        .unwrap_or_default();

    let model = extract_frontmatter_field(&identity_content, "model")
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let temperature: f64 = extract_frontmatter_field(&identity_content, "temperature")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.7);
    let max_tokens: u32 = extract_frontmatter_field(&identity_content, "max_tokens")
        .and_then(|s| s.parse().ok())
        .unwrap_or(4096);

    // Read SOUL.md for personality
    let soul_path = bot_dir.join("SOUL.md");
    let soul_content = tokio::fs::read_to_string(&soul_path)
        .await
        .unwrap_or_default();

    let tone = extract_frontmatter_field(&soul_content, "tone").unwrap_or_else(|| "neutral".to_string());

    let current_config = BuilderConfig {
        name: bot.name.clone(),
        description: bot.description.clone(),
        category: bot.category.to_string(),
        tags: bot.tags.clone(),
        personality: PersonalityConfig {
            tone,
            traits: vec![],
            purpose: bot.description.clone(),
            boundaries: None,
        },
        model_config: ModelConfig {
            model,
            temperature,
            max_tokens,
        },
        skills: vec![],
    };

    let session_id = Uuid::now_v7();
    let mut builder_state = new_builder_state(session_id, bot.description.clone());

    let provider = state
        .create_single_provider("claude-sonnet-4-20250514")
        .await
        .context("Failed to create LLM provider for builder")?;

    let builder = LlmBuilderAgent::new(
        provider,
        Some((*state.builder_memory_store).clone()),
        "claude-sonnet-4-20250514".to_string(),
    );

    println!();
    println!(
        "  {} Reconfiguring bot: {}",
        style("*").cyan().bold(),
        style(&bot.name).yellow()
    );

    let turn = builder
        .reconfigure(&mut builder_state, current_config)
        .await
        .context("Builder failed to start reconfiguration")?;

    run_conversation_loop(state, &builder, &mut builder_state, turn).await
}

// ---------------------------------------------------------------------------
// Conversation loop
// ---------------------------------------------------------------------------

/// The main conversation loop that drives the builder wizard.
///
/// Processes `BuilderTurn` variants: AskQuestion (dialoguer Select),
/// ShowPreview (live config display), ReadyToAssemble (confirmation + assembly),
/// and Clarify (free-text input).
///
/// Auto-saves drafts after each turn. Records builder memory after successful
/// assembly.
async fn run_conversation_loop(
    state: &AppState,
    builder: &LlmBuilderAgent<SqliteBuilderMemoryStore>,
    builder_state: &mut BuilderState,
    mut turn: BuilderTurn,
) -> Result<()> {
    loop {
        match turn {
            BuilderTurn::AskQuestion {
                phase: _,
                question,
                options,
                allow_free_text,
                phase_label,
            } => {
                // Show phase label if present
                if let Some(label) = phase_label {
                    println!("\n{}", style(label).dim());
                }

                // Show the question
                println!("\n{}", style(&question).bold());

                // Build dialoguer Select with options
                let mut items: Vec<String> = options
                    .iter()
                    .map(|o| {
                        if let Some(desc) = &o.description {
                            format!("{} -- {}", o.label, style(desc).dim())
                        } else {
                            o.label.clone()
                        }
                    })
                    .collect();

                // Add "Other (type your own)" and "< Back" options
                if allow_free_text {
                    items.push("Other (type your own)".to_string());
                }
                items.push("< Back".to_string());

                let selection = Select::new().items(&items).default(0).interact()?;

                let answer = if selection == items.len() - 1 {
                    // Last item is always "< Back"
                    BuilderAnswer::Back
                } else if allow_free_text && selection == items.len() - 2 {
                    // Second-to-last (when free text enabled) is "Other"
                    let text: String = Input::new()
                        .with_prompt("Your answer")
                        .interact_text()?;
                    BuilderAnswer::FreeText(text)
                } else {
                    BuilderAnswer::OptionIndex(selection)
                };

                turn = builder
                    .next_turn(builder_state, answer)
                    .await
                    .context("Builder failed to process answer")?;
            }

            BuilderTurn::ShowPreview { phase: _, preview } => {
                println!("\n{}", style("--- Current Configuration ---").cyan());
                if let Some(name) = &preview.name {
                    println!("  Name: {}", name);
                }
                if let Some(desc) = &preview.description {
                    println!("  Description: {}", desc);
                }
                if let Some(personality) = &preview.personality_summary {
                    println!("  Personality: {}", personality);
                }
                if let Some(model) = &preview.model {
                    println!("  Model: {}", model);
                }
                if !preview.skills.is_empty() {
                    println!("  Skills: {}", preview.skills.join(", "));
                }
                println!("{}", style("---").cyan());
                turn = builder
                    .next_turn(builder_state, BuilderAnswer::Confirm(true))
                    .await
                    .context("Builder failed after preview")?;
            }

            BuilderTurn::ReadyToAssemble { config } => {
                println!(
                    "\n{}",
                    style("=== Ready to Create ===").green().bold()
                );
                println!("Name: {}", config.name);
                println!("Description: {}", config.description);
                println!("Category: {}", config.category);
                println!(
                    "Model: {} (temp: {}, max: {})",
                    config.model_config.model,
                    config.model_config.temperature,
                    config.model_config.max_tokens
                );
                println!(
                    "Personality: {} tone, traits: {}",
                    config.personality.tone,
                    config.personality.traits.join(", ")
                );
                if !config.skills.is_empty() {
                    println!("Skills:");
                    for skill in &config.skills {
                        println!("  - {}: {}", skill.name, skill.description);
                    }
                }

                let confirmed = Confirm::new()
                    .with_prompt("Ready to create?")
                    .default(true)
                    .interact()?;

                if confirmed {
                    let result =
                        BotAssembler::assemble(&*state.bot_service, &config)
                            .await
                            .context("Bot assembly failed")?;

                    println!();
                    println!("{}", BotAssembler::format_assembly_summary(&result));

                    // Record builder memory for future suggestions
                    let memory_entry = BuilderMemoryEntry {
                        id: Uuid::now_v7(),
                        purpose_category: builder_state
                            .purpose_category
                            .as_ref()
                            .and_then(|c| serde_json::to_string(c).ok())
                            .unwrap_or_default(),
                        initial_description: builder_state.initial_description.clone(),
                        chosen_tone: Some(config.personality.tone.clone()),
                        chosen_model: Some(config.model_config.model.clone()),
                        chosen_skills: config
                            .skills
                            .iter()
                            .map(|s| s.name.clone())
                            .collect(),
                        bot_slug: Some(result.bot.slug.clone()),
                        created_at: chrono::Utc::now(),
                    };
                    let _ = state
                        .builder_memory_store
                        .record_session(memory_entry)
                        .await;

                    // Delete draft after successful creation
                    let _ = state
                        .builder_draft_store
                        .delete_draft(&builder_state.session_id)
                        .await;

                    let start_chat = Confirm::new()
                        .with_prompt("Start chatting with your new bot?")
                        .default(true)
                        .interact()?;

                    if start_chat {
                        println!(
                            "\nRun: {}",
                            style(format!("bnity chat {}", result.bot.slug)).yellow()
                        );
                    }
                } else {
                    turn = builder
                        .next_turn(builder_state, BuilderAnswer::Back)
                        .await
                        .context("Builder failed to go back from assembly")?;
                    continue;
                }
                break;
            }

            BuilderTurn::Clarify { message } => {
                println!("\n{}", style(&message).yellow());
                let clarification: String = Input::new()
                    .with_prompt("Your response")
                    .interact_text()?;
                turn = builder
                    .next_turn(builder_state, BuilderAnswer::FreeText(clarification))
                    .await
                    .context("Builder failed to process clarification")?;
            }
        }

        // Auto-save draft after each turn
        let draft = BuilderDraft {
            session_id: builder_state.session_id,
            state_json: serde_json::to_string(builder_state).unwrap_or_default(),
            schema_version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let _ = state.builder_draft_store.save_draft(draft).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a field from YAML frontmatter in a Markdown file.
fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return None;
    }
    let end = lines
        .iter()
        .skip(1)
        .position(|l| l.trim() == "---")
        .map(|p| p + 1)?;

    for line in &lines[1..end] {
        if let Some(value) = line.strip_prefix(&format!("{field}:")) {
            return Some(value.trim().to_string());
        }
    }
    None
}
