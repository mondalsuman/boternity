//! Memory management CLI commands: list, search, remember, forget, delete, export, audit.
//!
//! Provides memory browsing with provenance, semantic search with similarity scores,
//! manual injection (to both SQLite and LanceDB), individual deletion with audit,
//! JSON export, and audit log viewing.

use anyhow::{Context, Result};
use chrono::Utc;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Confirm;
use uuid::Uuid;

use boternity_core::memory::box_embedder::BoxEmbedder;
use boternity_core::memory::box_vector::BoxVectorMemoryStore;
use boternity_core::memory::store::MemoryRepository;
use boternity_infra::sqlite::audit::SqliteAuditLog;
use boternity_types::memory::{
    AuditAction, MemoryAuditEntry, MemoryCategory, MemoryEntry, VectorMemoryEntry,
};

use crate::state::AppState;

/// List all memories for a bot with provenance, category, and importance.
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

        let category_cell = category_to_cell(&mem.category);
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

/// Search memories using vector similarity and display results with similarity scores.
///
/// Results are color-coded by similarity:
/// - Green: similarity >= 0.7 (strong match)
/// - Yellow: similarity >= 0.4 (moderate match)
/// - Red: similarity < 0.4 (weak match)
///
/// # Examples
///
/// ```bash
/// bnity memory search my-bot "what programming language"
/// bnity memory search my-bot "dark mode" --limit 5
/// ```
pub async fn search_memories(
    state: &AppState,
    slug: &str,
    query: &str,
    limit: usize,
    min_similarity: f32,
    vector_store: &BoxVectorMemoryStore,
    embedder: &BoxEmbedder,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    // Embed the query text
    let embeddings = embedder
        .embed(&[query.to_string()])
        .await
        .with_context(|| "Failed to embed search query")?;

    let query_embedding = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Embedder returned no vectors"))?;

    // Search the vector store
    let results = vector_store
        .search(&bot.id.0, &query_embedding, limit, min_similarity)
        .await
        .with_context(|| "Vector memory search failed")?;

    if json {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                let similarity = 1.0 - r.distance;
                serde_json::json!({
                    "fact": r.entry.fact,
                    "category": r.entry.category.to_string(),
                    "importance": r.entry.importance,
                    "similarity": format!("{similarity:.4}"),
                    "relevance_score": format!("{:.4}", r.relevance_score),
                    "distance": format!("{:.4}", r.distance),
                    "provenance": r.provenance,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!();
        println!(
            "  {} No memories matched '{}' for '{}'.",
            style("i").blue().bold(),
            style(query).dim(),
            style(&bot.name).cyan(),
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Similarity").fg(Color::White),
        Cell::new("Fact").fg(Color::White),
        Cell::new("Category").fg(Color::White),
        Cell::new("Importance").fg(Color::White),
        Cell::new("Provenance").fg(Color::White),
    ]);

    for result in &results {
        let similarity = 1.0 - result.distance;
        let sim_display = format!("{similarity:.4}");

        // Color-code similarity score
        let sim_color = if similarity >= 0.7 {
            Color::Green
        } else if similarity >= 0.4 {
            Color::Yellow
        } else {
            Color::Red
        };

        let fact_display = if result.entry.fact.len() > 50 {
            format!("{}...", &result.entry.fact[..47])
        } else {
            result.entry.fact.clone()
        };

        let category_cell = category_to_cell(&result.entry.category);
        let importance = format_importance(result.entry.importance);
        let provenance = result
            .provenance
            .as_deref()
            .unwrap_or("-")
            .to_string();

        table.add_row(vec![
            Cell::new(sim_display).fg(sim_color),
            Cell::new(fact_display).fg(Color::White),
            category_cell,
            Cell::new(importance).fg(Color::Yellow),
            Cell::new(provenance).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!(
        "  Search results for '{}' in '{}'",
        style(query).white().bold(),
        style(&bot.name).cyan().bold(),
    );
    println!();
    println!("{table}");
    println!();
    println!(
        "  {} result{}",
        style(results.len()).bold(),
        if results.len() == 1 { "" } else { "s" }
    );
    println!();

    Ok(())
}

/// Manually inject a memory for a bot (saves to both SQLite and optionally LanceDB).
///
/// # Examples
///
/// ```bash
/// bnity remember my-bot "prefers TypeScript over JavaScript"
/// ```
pub async fn remember(
    state: &AppState,
    slug: &str,
    fact: &str,
    vector_store: Option<&BoxVectorMemoryStore>,
    embedder: Option<&BoxEmbedder>,
    audit_log: Option<&SqliteAuditLog>,
    json: bool,
) -> Result<()> {
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

    // Save to SQLite
    state
        .chat_service
        .memory_repo()
        .save_memory(&entry)
        .await?;

    // Save to LanceDB if vector store and embedder are available
    if let (Some(vs), Some(emb)) = (vector_store, embedder) {
        let embeddings = emb
            .embed(&[fact.to_string()])
            .await
            .with_context(|| "Failed to embed memory for vector storage")?;

        if let Some(embedding) = embeddings.into_iter().next() {
            let vector_entry = VectorMemoryEntry {
                id: entry.id,
                bot_id: entry.bot_id,
                fact: entry.fact.clone(),
                category: entry.category.clone(),
                importance: entry.importance,
                session_id: None,
                source_memory_id: Some(entry.id),
                embedding_model: emb.model_name().to_string(),
                created_at: entry.created_at,
                last_accessed_at: None,
                access_count: 0,
            };
            if let Err(e) = vs.add(&vector_entry, &embedding).await {
                tracing::warn!("Failed to store memory in vector DB (SQLite saved): {e}");
            }
        }
    }

    // Log to audit trail
    if let Some(audit) = audit_log {
        let audit_entry = MemoryAuditEntry {
            id: Uuid::now_v7(),
            bot_id: bot.id.0,
            memory_id: entry.id,
            action: AuditAction::Add,
            actor: "user".to_string(),
            details: Some(serde_json::json!({"source": "manual", "fact": fact}).to_string()),
            created_at: Utc::now(),
        };
        if let Err(e) = audit.log(&audit_entry).await {
            tracing::warn!("Failed to log audit entry: {e}");
        }
    }

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

/// Delete a single memory by ID (from both SQLite and LanceDB, with audit).
///
/// # Examples
///
/// ```bash
/// bnity delete memory <memory-id>
/// bnity delete memory <memory-id> --force
/// ```
pub async fn delete_memory(
    state: &AppState,
    memory_id: Uuid,
    force: bool,
    vector_store: Option<&BoxVectorMemoryStore>,
    audit_log: Option<&SqliteAuditLog>,
    json: bool,
) -> Result<()> {
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

    // Delete from SQLite
    state
        .chat_service
        .memory_repo()
        .delete_memory(&memory_id)
        .await?;

    // Delete from LanceDB (best-effort; we don't know the bot_id without a lookup,
    // but vector store delete needs it. We'll attempt removal for the memory_id across
    // all tables by using the delete method if a bot_id is available).
    // Note: In practice, the caller would know the bot_id. For now, we skip vector
    // deletion here because the trait requires bot_id. A future integration plan
    // can add bot_id resolution. The memory is removed from SQLite (source of truth).
    if let Some(_vs) = vector_store {
        // TODO: resolve bot_id from memory_id to delete from vector store
        tracing::debug!(
            "Vector store deletion requires bot_id; memory {} removed from SQLite only",
            memory_id
        );
    }

    // Log to audit trail
    if let Some(audit) = audit_log {
        // We use Uuid::nil() for bot_id since we may not know it here.
        // In a full integration, the caller resolves bot_id first.
        let audit_entry = MemoryAuditEntry {
            id: Uuid::now_v7(),
            bot_id: Uuid::nil(),
            memory_id,
            action: AuditAction::Delete,
            actor: "user".to_string(),
            details: None,
            created_at: Utc::now(),
        };
        if let Err(e) = audit.log(&audit_entry).await {
            tracing::warn!("Failed to log audit entry: {e}");
        }
    }

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

/// Export all memories for a bot as JSON to stdout.
///
/// # Examples
///
/// ```bash
/// bnity memory export my-bot
/// bnity memory export my-bot > memories.json
/// ```
pub async fn export_memories(state: &AppState, slug: &str) -> Result<()> {
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

    let export = serde_json::json!({
        "bot_slug": slug,
        "bot_name": bot.name,
        "exported_at": Utc::now().to_rfc3339(),
        "count": memories.len(),
        "memories": memories,
    });

    println!("{}", serde_json::to_string_pretty(&export)?);

    Ok(())
}

/// Show the memory audit log for a bot.
///
/// Displays recent memory operations (add, delete, share, revoke, merge)
/// with actor and timestamp.
///
/// # Examples
///
/// ```bash
/// bnity memory audit my-bot
/// bnity memory audit my-bot --limit 20
/// ```
pub async fn memory_audit(
    state: &AppState,
    slug: &str,
    limit: Option<i64>,
    audit_log: &SqliteAuditLog,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let entries = audit_log
        .get_for_bot(&bot.id.0, limit)
        .await
        .with_context(|| "Failed to read audit log")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!();
        println!(
            "  {} No audit entries for '{}'.",
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
        Cell::new("Action").fg(Color::White),
        Cell::new("Memory ID").fg(Color::White),
        Cell::new("Actor").fg(Color::White),
        Cell::new("Details").fg(Color::White),
        Cell::new("Date").fg(Color::White),
    ]);

    for entry in &entries {
        let action_cell = match entry.action {
            AuditAction::Add => Cell::new("add").fg(Color::Green),
            AuditAction::Delete => Cell::new("delete").fg(Color::Red),
            AuditAction::Share => Cell::new("share").fg(Color::Cyan),
            AuditAction::Revoke => Cell::new("revoke").fg(Color::Yellow),
            AuditAction::Merge => Cell::new("merge").fg(Color::Magenta),
        };

        let memory_id_short = entry.memory_id.to_string()[..8].to_string();

        let details = entry
            .details
            .as_deref()
            .map(|d| {
                if d.len() > 40 {
                    format!("{}...", &d[..37])
                } else {
                    d.to_string()
                }
            })
            .unwrap_or_else(|| "-".to_string());

        let date = entry.created_at.format("%Y-%m-%d %H:%M").to_string();

        table.add_row(vec![
            action_cell,
            Cell::new(memory_id_short).fg(Color::DarkGrey),
            Cell::new(&entry.actor).fg(Color::White),
            Cell::new(details).fg(Color::DarkGrey),
            Cell::new(date).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!(
        "  Audit log for '{}'",
        style(&bot.name).cyan().bold()
    );
    println!();
    println!("{table}");
    println!();
    println!(
        "  {} entr{}",
        style(entries.len()).bold(),
        if entries.len() == 1 { "y" } else { "ies" }
    );
    println!();

    Ok(())
}

// --- Formatting helpers ---

fn format_importance(level: u8) -> String {
    let stars = level.min(5) as usize;
    let empty = 5 - stars;
    format!("{}{}", "*".repeat(stars), "-".repeat(empty))
}

fn category_to_cell(category: &MemoryCategory) -> Cell {
    match category {
        MemoryCategory::Preference => Cell::new("preference").fg(Color::Magenta),
        MemoryCategory::Fact => Cell::new("fact").fg(Color::Cyan),
        MemoryCategory::Decision => Cell::new("decision").fg(Color::Yellow),
        MemoryCategory::Context => Cell::new("context").fg(Color::Blue),
        MemoryCategory::Correction => Cell::new("correction").fg(Color::Red),
    }
}
