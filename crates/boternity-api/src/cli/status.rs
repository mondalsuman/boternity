//! System status dashboard command.

use anyhow::Result;
use console::style;

use boternity_types::bot::BotStatus;
use boternity_types::secret::SecretScope;

use crate::state::AppState;

/// Display system status dashboard.
///
/// Shows bot counts by status, storage info, secrets count, and version.
pub async fn status(state: &AppState, json: bool) -> Result<()> {
    // Gather stats
    let all_bots = state.bot_service.list_bots(None).await?;
    let active = all_bots
        .iter()
        .filter(|b| b.status == BotStatus::Active)
        .count();
    let disabled = all_bots
        .iter()
        .filter(|b| b.status == BotStatus::Disabled)
        .count();
    let archived = all_bots
        .iter()
        .filter(|b| b.status == BotStatus::Archived)
        .count();

    let total_tokens: i64 = all_bots.iter().map(|b| b.total_tokens_used).sum();
    let total_conversations: i64 = all_bots.iter().map(|b| b.conversation_count).sum();

    let secrets = state
        .secret_service
        .list_secrets(&SecretScope::Global)
        .await
        .unwrap_or_default();

    if json {
        let status = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "data_dir": state.data_dir.display().to_string(),
            "bots": {
                "total": all_bots.len(),
                "active": active,
                "disabled": disabled,
                "archived": archived,
            },
            "secrets": secrets.len(),
            "total_tokens": total_tokens,
            "total_conversations": total_conversations,
        });
        println!("{}", serde_json::to_string_pretty(&status)?);
        return Ok(());
    }

    println!();
    println!(
        "  {} Boternity v{}",
        style("⚡").bold(),
        env!("CARGO_PKG_VERSION")
    );
    println!();

    // Bot counts
    println!("  {}", style("── Bots ──").dim());
    println!(
        "  Total:    {}",
        style(all_bots.len()).bold()
    );
    println!(
        "  Active:   {}",
        style(active).green()
    );
    if disabled > 0 {
        println!(
            "  Disabled: {}",
            style(disabled).yellow()
        );
    }
    if archived > 0 {
        println!(
            "  Archived: {}",
            style(archived).dim()
        );
    }
    println!();

    // Usage stats
    println!("  {}", style("── Usage ──").dim());
    println!(
        "  Conversations: {}",
        total_conversations
    );
    println!(
        "  Tokens used:   {}",
        format_tokens(total_tokens)
    );
    println!();

    // Secrets
    println!("  {}", style("── Secrets ──").dim());
    println!(
        "  Stored: {}",
        style(secrets.len()).bold()
    );
    println!();

    // System
    println!("  {}", style("── System ──").dim());
    println!(
        "  Data dir: {}",
        style(state.data_dir.display()).dim()
    );
    println!(
        "  Database: {}",
        style("SQLite (WAL mode)").dim()
    );
    println!();

    Ok(())
}

fn format_tokens(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
