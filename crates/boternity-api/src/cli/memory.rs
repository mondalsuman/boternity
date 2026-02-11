//! Memory management CLI commands: list, remember, forget, delete.
//!
//! Provides memory browsing with provenance, manual injection,
//! individual deletion, and full wipe with confirmation.

use anyhow::{Context, Result};
use chrono::Utc;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Confirm;
use uuid::Uuid;

use boternity_core::memory::store::MemoryRepository;
use boternity_types::memory::{MemoryCategory, MemoryEntry};

use crate::state::AppState;

/// List all memories for a bot with provenance information.
///
/// # Examples
///
/// ```bash
/// bnity memories my-bot
/// bnity memories my-bot --json
/// ```
pub async fn list_memories(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let memories = state
        .chat_service
        .memory_repo()
        .get_memories(&bot.id.0, None)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&memories)?);
        return Ok(());
    }

    if memories.is_empty() {
        println!();
        println!(
            "  {} No memories for '{}'. Memories are extracted from conversations.",
            style("i").blue().bold(),
            style(&bot.name).cyan(),
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Fact").fg(Color::White),
        Cell::new("Category").fg(Color::White),
        Cell::new("Importance").fg(Color::White),
        Cell::new("Source").fg(Color::White),
        Cell::new("Date").fg(Color::White),
    ]);

    for mem in &memories {
        let fact_display = if mem.fact.len() > 60 {
            format!("{}...", &mem.fact[..57])
        } else {
            mem.fact.clone()
        };

        let category_cell = match &mem.category {
            MemoryCategory::Preference => Cell::new("preference").fg(Color::Magenta),
            MemoryCategory::Fact => Cell::new("fact").fg(Color::Cyan),
            MemoryCategory::Decision => Cell::new("decision").fg(Color::Yellow),
            MemoryCategory::Context => Cell::new("context").fg(Color::Blue),
            MemoryCategory::Correction => Cell::new("correction").fg(Color::Red),
        };

        let importance = format_importance(mem.importance);

        let source = if mem.is_manual {
            "manual".to_string()
        } else {
            format!("session:{}", &mem.session_id.to_string()[..8])
        };

        let date = mem.created_at.format("%Y-%m-%d").to_string();

        table.add_row(vec![
            Cell::new(fact_display).fg(Color::White),
            category_cell,
            Cell::new(importance).fg(Color::Yellow),
            Cell::new(source).fg(Color::DarkGrey),
            Cell::new(date).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!(
        "  Memories for '{}'",
        style(&bot.name).cyan().bold()
    );
    println!();
    println!("{table}");
    println!();
    println!(
        "  {} memor{}",
        style(memories.len()).bold(),
        if memories.len() == 1 { "y" } else { "ies" }
    );
    println!();

    Ok(())
}

/// Manually inject a memory for a bot.
///
/// # Examples
///
/// ```bash
/// bnity remember my-bot "prefers TypeScript over JavaScript"
/// ```
pub async fn remember(state: &AppState, slug: &str, fact: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let entry = MemoryEntry {
        id: Uuid::now_v7(),
        bot_id: bot.id.0,
        // Manual memories use a nil session ID (not linked to a session)
        session_id: Uuid::nil(),
        fact: fact.to_string(),
        category: MemoryCategory::Fact,
        importance: 3,
        source_message_id: None,
        superseded_by: None,
        created_at: Utc::now(),
        is_manual: true,
    };

    state
        .chat_service
        .memory_repo()
        .save_memory(&entry)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entry)?);
    } else {
        println!(
            "  {} Memory saved for '{}'",
            style("*").green().bold(),
            style(&bot.name).cyan()
        );
        println!(
            "  {}",
            style(fact).dim()
        );
    }

    Ok(())
}

/// Delete all memories for a bot with confirmation.
///
/// # Examples
///
/// ```bash
/// bnity forget my-bot
/// bnity forget my-bot --force
/// ```
pub async fn forget(state: &AppState, slug: &str, force: bool, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let memories = state
        .chat_service
        .memory_repo()
        .get_memories(&bot.id.0, None)
        .await?;

    if memories.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::json!({"deleted": 0, "bot": slug})
            );
        } else {
            println!(
                "  {} No memories to delete for '{}'.",
                style("i").blue().bold(),
                style(&bot.name).cyan()
            );
        }
        return Ok(());
    }

    if !force && !json {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Wipe all {} memories for '{}'? This cannot be undone.",
                style(memories.len()).bold(),
                style(&bot.name).red().bold()
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    let count = state
        .chat_service
        .memory_repo()
        .delete_all_memories(&bot.id.0)
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({"deleted": count, "bot": slug})
        );
    } else {
        println!(
            "  {} Wiped {} memor{} for '{}'.",
            style("x").red().bold(),
            count,
            if count == 1 { "y" } else { "ies" },
            bot.name
        );
    }

    Ok(())
}

/// Delete a single memory by ID.
///
/// # Examples
///
/// ```bash
/// bnity delete memory <memory-id>
/// bnity delete memory <memory-id> --force
/// ```
pub async fn delete_memory(state: &AppState, memory_id: Uuid, force: bool, json: bool) -> Result<()> {
    // We don't have a get_memory_by_id, so attempt delete directly.
    // The repository will silently succeed even if the ID doesn't exist,
    // but that's acceptable for a delete operation.

    if !force && !json {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Delete memory {}?",
                style(&memory_id.to_string()[..8]).red().bold()
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    state
        .chat_service
        .memory_repo()
        .delete_memory(&memory_id)
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({"deleted": true, "memory_id": memory_id.to_string()})
        );
    } else {
        println!(
            "  {} Memory {} deleted.",
            style("x").red().bold(),
            &memory_id.to_string()[..8]
        );
    }

    Ok(())
}

// --- Formatting helpers ---

fn format_importance(level: u8) -> String {
    let stars = level.min(5) as usize;
    let empty = 5 - stars;
    format!("{}{}", "*".repeat(stars), "-".repeat(empty))
}
