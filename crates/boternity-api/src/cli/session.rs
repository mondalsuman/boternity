//! Session management CLI commands: list, export, delete.
//!
//! Provides session browsing with rich tables, Markdown/JSON export,
//! and deletion with confirmation prompt.

use anyhow::{Context, Result};
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Confirm;
use uuid::Uuid;

use boternity_core::chat::repository::ChatRepository;

use crate::state::AppState;

/// List past sessions for a bot with date, duration, title, and message preview.
///
/// # Examples
///
/// ```bash
/// bnity sessions my-bot
/// bnity sessions my-bot --json
/// ```
pub async fn list_sessions(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let sessions = state
        .chat_service
        .list_sessions(&bot.id.0, None, None)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!();
        println!(
            "  {} No sessions found for '{}'. Start one with: {}",
            style("i").blue().bold(),
            style(&bot.name).cyan(),
            style(format!("bnity chat {}", bot.slug)).yellow()
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Title").fg(Color::White),
        Cell::new("Started").fg(Color::White),
        Cell::new("Duration").fg(Color::White),
        Cell::new("Messages").fg(Color::White),
        Cell::new("Status").fg(Color::White),
    ]);

    for session in &sessions {
        let title = session
            .title
            .as_deref()
            .unwrap_or("(untitled)")
            .to_string();

        let title_display = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title
        };

        let started = session.started_at.format("%Y-%m-%d %H:%M").to_string();

        let duration = match session.ended_at {
            Some(ended) => format_duration(ended - session.started_at),
            None => "ongoing".to_string(),
        };

        let status_cell = match &session.status {
            boternity_types::chat::SessionStatus::Active => {
                Cell::new("active").fg(Color::Green)
            }
            boternity_types::chat::SessionStatus::Completed => {
                Cell::new("completed").fg(Color::DarkGrey)
            }
            boternity_types::chat::SessionStatus::Crashed => {
                Cell::new("crashed").fg(Color::Red)
            }
        };

        table.add_row(vec![
            Cell::new(title_display).fg(Color::Cyan),
            Cell::new(started).fg(Color::White),
            Cell::new(duration).fg(Color::DarkGrey),
            Cell::new(session.message_count.to_string()).fg(Color::White),
            status_cell,
        ]);
    }

    println!();
    println!(
        "  Sessions for '{}'",
        style(&bot.name).cyan().bold()
    );
    println!();
    println!("{table}");
    println!();
    println!(
        "  {} session{}",
        style(sessions.len()).bold(),
        if sessions.len() == 1 { "" } else { "s" }
    );
    println!();

    Ok(())
}

/// Export a session as Markdown (default) or JSON.
///
/// # Examples
///
/// ```bash
/// bnity export session <session-id>
/// bnity export session <session-id> --json
/// ```
pub async fn export_session(state: &AppState, session_id: Uuid, json: bool) -> Result<()> {
    let session = state
        .chat_service
        .get_session(&session_id)
        .await?
        .with_context(|| format!("Session '{session_id}' not found"))?;

    let messages = state
        .chat_service
        .get_messages(&session_id, None, None)
        .await?;

    if json {
        let export = serde_json::json!({
            "session": session,
            "messages": messages,
        });
        println!("{}", serde_json::to_string_pretty(&export)?);
        return Ok(());
    }

    // Markdown export
    let title = session.title.as_deref().unwrap_or("Untitled Session");

    println!("# {title}");
    println!();
    println!(
        "- **Started:** {}",
        session.started_at.format("%Y-%m-%d %H:%M UTC")
    );
    if let Some(ended) = session.ended_at {
        println!(
            "- **Ended:** {}",
            ended.format("%Y-%m-%d %H:%M UTC")
        );
        println!(
            "- **Duration:** {}",
            format_duration(ended - session.started_at)
        );
    }
    println!("- **Messages:** {}", session.message_count);
    println!("- **Model:** {}", session.model);
    println!(
        "- **Tokens:** {} in / {} out",
        session.total_input_tokens, session.total_output_tokens
    );
    println!();
    println!("---");
    println!();

    for msg in &messages {
        let role_label = match msg.role {
            boternity_types::llm::MessageRole::User => "**You**",
            boternity_types::llm::MessageRole::Assistant => "**Assistant**",
            boternity_types::llm::MessageRole::System => "**System**",
        };

        let timestamp = msg.created_at.format("%H:%M");
        println!("### {role_label} ({timestamp})");
        println!();
        println!("{}", msg.content);
        println!();
    }

    Ok(())
}

/// Delete a session with confirmation.
///
/// # Examples
///
/// ```bash
/// bnity delete session <session-id>
/// bnity delete session <session-id> --force
/// ```
pub async fn delete_session(state: &AppState, session_id: Uuid, force: bool, json: bool) -> Result<()> {
    let session = state
        .chat_service
        .get_session(&session_id)
        .await?
        .with_context(|| format!("Session '{session_id}' not found"))?;

    let title = session
        .title
        .as_deref()
        .unwrap_or("(untitled)")
        .to_string();

    if !force && !json {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Delete session '{}' ({} messages)?",
                style(&title).red().bold(),
                session.message_count
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
        .chat_repo()
        .delete_session(&session_id)
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({"deleted": true, "session_id": session_id.to_string()})
        );
    } else {
        println!(
            "  {} Session '{}' deleted.",
            style("x").red().bold(),
            title
        );
    }

    Ok(())
}

// --- Formatting helpers ---

fn format_duration(duration: chrono::TimeDelta) -> String {
    let total_secs = duration.num_seconds().max(0);
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;

    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else if mins > 0 {
        format!("{}m", mins)
    } else {
        format!("{}s", total_secs)
    }
}
