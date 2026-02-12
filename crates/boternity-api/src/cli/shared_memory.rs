//! Shared memory CLI commands: search, list, share, revoke, details.
//!
//! Provides a dedicated `bnity shared-memory` subcommand for browsing and managing
//! cross-bot shared memories with trust-level filtering and provenance tracking.
//! This is intentionally separate from `bnity memory` (per-bot private memories).

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use uuid::Uuid;

use boternity_core::memory::box_embedder::BoxEmbedder;
use boternity_infra::sqlite::audit::SqliteAuditLog;
use boternity_infra::vector::shared::LanceSharedMemoryStore;
use boternity_types::memory::{AuditAction, MemoryAuditEntry, TrustLevel};

use boternity_core::memory::shared::SharedMemoryStore;

use crate::state::AppState;

/// Shared memory management subcommands.
///
/// Accessed via `bnity shared-memory <action>`.
#[derive(Subcommand)]
pub enum SharedMemoryCommand {
    /// Search shared memories visible to a bot (filtered by trust level).
    Search {
        /// Bot slug performing the search (determines visibility).
        slug: String,

        /// Search query text.
        query: String,

        /// Maximum number of results.
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Minimum similarity threshold (0.0 to 1.0).
        #[arg(long, default_value = "0.3")]
        min_similarity: f32,
    },

    /// List shared memories visible to a bot with provenance.
    List {
        /// Bot slug performing the listing (determines visibility).
        slug: String,

        /// Maximum number of results.
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Share a memory (change trust level to public or trusted).
    Share {
        /// Memory ID to share.
        id: String,

        /// Trust level to set (public or trusted).
        #[arg(long, default_value = "public")]
        level: String,
    },

    /// Revoke sharing of a memory (sets back to private).
    Revoke {
        /// Memory ID to revoke.
        id: String,

        /// Bot slug of the author (for authorship verification).
        slug: String,
    },

    /// Show details of a specific shared memory by ID.
    Details {
        /// Memory ID to inspect.
        id: String,
    },
}

/// Handle shared memory subcommand dispatch.
pub async fn handle_shared_memory_command(
    command: SharedMemoryCommand,
    state: &AppState,
    shared_store: &LanceSharedMemoryStore,
    embedder: &BoxEmbedder,
    audit_log: &SqliteAuditLog,
    json: bool,
) -> Result<()> {
    match command {
        SharedMemoryCommand::Search {
            slug,
            query,
            limit,
            min_similarity,
        } => {
            search_shared_memories(
                state,
                &slug,
                &query,
                limit,
                min_similarity,
                shared_store,
                embedder,
                json,
            )
            .await
        }

        SharedMemoryCommand::List { slug, limit } => {
            list_shared_memories(state, &slug, limit, shared_store, embedder, json).await
        }

        SharedMemoryCommand::Share { id, level } => {
            share_memory(&id, &level, shared_store, audit_log, json).await
        }

        SharedMemoryCommand::Revoke { id, slug } => {
            revoke_memory(state, &id, &slug, shared_store, audit_log, json).await
        }

        SharedMemoryCommand::Details { id } => {
            memory_details(&id, shared_store, json).await
        }
    }
}

/// Search shared memories visible to a bot with trust-level filtering.
///
/// Results show similarity scores (color-coded) and provenance (author bot name).
async fn search_shared_memories(
    state: &AppState,
    slug: &str,
    query: &str,
    limit: usize,
    min_similarity: f32,
    shared_store: &LanceSharedMemoryStore,
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

    // TODO: Load trusted_bot_ids from bot's trust list config
    // For now, search with an empty trust list (sees public + own memories only)
    let trusted_bot_ids: Vec<Uuid> = Vec::new();

    let results = shared_store
        .search(
            &bot.id.0,
            &trusted_bot_ids,
            &query_embedding,
            limit,
            min_similarity,
        )
        .await
        .with_context(|| "Shared memory search failed")?;

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
            "  {} No shared memories matched '{}' for '{}'.",
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
        Cell::new("Author").fg(Color::White),
        Cell::new("Importance").fg(Color::White),
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

        let fact_display = if result.entry.fact.len() > 45 {
            format!("{}...", &result.entry.fact[..42])
        } else {
            result.entry.fact.clone()
        };

        let author = result
            .provenance
            .as_deref()
            .unwrap_or("unknown");

        let importance = format_importance(result.entry.importance);

        let category = result.entry.category.to_string();

        table.add_row(vec![
            Cell::new(sim_display).fg(sim_color),
            Cell::new(fact_display).fg(Color::White),
            Cell::new(category).fg(Color::Cyan),
            Cell::new(author).fg(Color::Magenta),
            Cell::new(importance).fg(Color::Yellow),
        ]);
    }

    println!();
    println!(
        "  Shared memory search for '{}' (as '{}')",
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

/// List shared memories visible to a bot, showing provenance and trust level.
///
/// Uses a "dummy" embedding (zero vector) to list by proximity. For listing,
/// this effectively returns memories ordered by their natural storage order.
async fn list_shared_memories(
    state: &AppState,
    slug: &str,
    limit: usize,
    shared_store: &LanceSharedMemoryStore,
    embedder: &BoxEmbedder,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    // Use a zero embedding to get all memories (no semantic filtering)
    let zero_embedding = vec![0.0_f32; embedder.dimension()];

    // Empty trust list for now (sees public + own only)
    let trusted_bot_ids: Vec<Uuid> = Vec::new();

    let results = shared_store
        .search(
            &bot.id.0,
            &trusted_bot_ids,
            &zero_embedding,
            limit,
            0.0, // No min similarity for listing
        )
        .await
        .with_context(|| "Failed to list shared memories")?;

    if json {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.entry.id.to_string(),
                    "fact": r.entry.fact,
                    "category": r.entry.category.to_string(),
                    "importance": r.entry.importance,
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
            "  {} No shared memories visible to '{}'.",
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
        Cell::new("ID").fg(Color::White),
        Cell::new("Fact").fg(Color::White),
        Cell::new("Category").fg(Color::White),
        Cell::new("Author").fg(Color::White),
        Cell::new("Importance").fg(Color::White),
    ]);

    for result in &results {
        let id_short = result.entry.id.to_string()[..8].to_string();

        let fact_display = if result.entry.fact.len() > 45 {
            format!("{}...", &result.entry.fact[..42])
        } else {
            result.entry.fact.clone()
        };

        let author = result
            .provenance
            .as_deref()
            .unwrap_or("unknown");

        let importance = format_importance(result.entry.importance);
        let category = result.entry.category.to_string();

        table.add_row(vec![
            Cell::new(id_short).fg(Color::DarkGrey),
            Cell::new(fact_display).fg(Color::White),
            Cell::new(category).fg(Color::Cyan),
            Cell::new(author).fg(Color::Magenta),
            Cell::new(importance).fg(Color::Yellow),
        ]);
    }

    println!();
    println!(
        "  Shared memories visible to '{}'",
        style(&bot.name).cyan().bold(),
    );
    println!();
    println!("{table}");
    println!();
    println!(
        "  {} memor{}",
        style(results.len()).bold(),
        if results.len() == 1 { "y" } else { "ies" }
    );
    println!();

    Ok(())
}

/// Share a memory by changing its trust level (with audit logging).
async fn share_memory(
    id: &str,
    level: &str,
    shared_store: &LanceSharedMemoryStore,
    audit_log: &SqliteAuditLog,
    json: bool,
) -> Result<()> {
    let memory_id = Uuid::parse_str(id)
        .map_err(|_| anyhow::anyhow!("Invalid memory ID: {id}"))?;

    let trust_level: TrustLevel = level
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{e}"))?;

    if trust_level == TrustLevel::Private {
        return Err(anyhow::anyhow!(
            "Use 'revoke' to set a memory to private, not 'share'"
        ));
    }

    shared_store
        .share(&memory_id, trust_level.clone())
        .await
        .with_context(|| "Failed to share memory")?;

    // Audit log
    let audit_entry = MemoryAuditEntry {
        id: Uuid::now_v7(),
        bot_id: Uuid::nil(), // Author not known at this level
        memory_id,
        action: AuditAction::Share,
        actor: "user".to_string(),
        details: Some(
            serde_json::json!({"new_trust_level": trust_level.to_string()}).to_string(),
        ),
        created_at: Utc::now(),
    };
    if let Err(e) = audit_log.log(&audit_entry).await {
        tracing::warn!("Failed to log share audit entry: {e}");
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "shared": true,
                "memory_id": memory_id.to_string(),
                "trust_level": trust_level.to_string()
            })
        );
    } else {
        println!(
            "  {} Memory {} shared as '{}'.",
            style("*").green().bold(),
            style(&memory_id.to_string()[..8]).cyan(),
            style(trust_level.to_string()).yellow()
        );
    }

    Ok(())
}

/// Revoke sharing of a memory (set back to private, with audit logging).
async fn revoke_memory(
    state: &AppState,
    id: &str,
    slug: &str,
    shared_store: &LanceSharedMemoryStore,
    audit_log: &SqliteAuditLog,
    json: bool,
) -> Result<()> {
    let memory_id = Uuid::parse_str(id)
        .map_err(|_| anyhow::anyhow!("Invalid memory ID: {id}"))?;

    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    shared_store
        .revoke(&memory_id, &bot.id.0)
        .await
        .with_context(|| "Failed to revoke memory sharing (are you the author?)")?;

    // Audit log
    let audit_entry = MemoryAuditEntry {
        id: Uuid::now_v7(),
        bot_id: bot.id.0,
        memory_id,
        action: AuditAction::Revoke,
        actor: "user".to_string(),
        details: Some(
            serde_json::json!({"revoked_by": slug, "new_trust_level": "private"}).to_string(),
        ),
        created_at: Utc::now(),
    };
    if let Err(e) = audit_log.log(&audit_entry).await {
        tracing::warn!("Failed to log revoke audit entry: {e}");
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "revoked": true,
                "memory_id": memory_id.to_string(),
                "bot": slug
            })
        );
    } else {
        println!(
            "  {} Memory {} revoked (now private).",
            style("x").red().bold(),
            style(&memory_id.to_string()[..8]).cyan()
        );
    }

    Ok(())
}

/// Show details of a specific shared memory entry.
async fn memory_details(
    id: &str,
    shared_store: &LanceSharedMemoryStore,
    json: bool,
) -> Result<()> {
    let memory_id = Uuid::parse_str(id)
        .map_err(|_| anyhow::anyhow!("Invalid memory ID: {id}"))?;

    // Verify integrity as a way to check the memory exists and get its details
    let is_valid = shared_store
        .verify_integrity(&memory_id)
        .await
        .with_context(|| format!("Shared memory '{id}' not found"))?;

    // To get the full entry, we do a search from the author's perspective.
    // Since we don't know the author, we use Uuid::nil() which will still
    // see it if it's public (and we'll see it if it's in the results).
    //
    // Note: A proper implementation would add a get_by_id() method to the
    // SharedMemoryStore trait. For now, we report integrity status.

    if json {
        println!(
            "{}",
            serde_json::json!({
                "memory_id": memory_id.to_string(),
                "integrity_valid": is_valid,
            })
        );
    } else {
        println!();
        println!(
            "  Shared memory {}",
            style(&memory_id.to_string()[..8]).cyan().bold()
        );
        println!();
        let integrity_msg = if is_valid {
            format!("{}", style("PASSED").green())
        } else {
            format!("{}", style("FAILED").red())
        };
        println!("  Integrity check: {integrity_msg}");
        println!();
    }

    Ok(())
}

// --- Formatting helpers ---

fn format_importance(level: u8) -> String {
    let stars = level.min(5) as usize;
    let empty = 5 - stars;
    format!("{}{}", "*".repeat(stars), "-".repeat(empty))
}
