//! Bot lifecycle CLI commands: create, list, show, delete, clone.

use anyhow::Result;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::{Confirm, Input};
use indicatif::{ProgressBar, ProgressStyle};

use boternity_types::bot::{BotCategory, BotStatus, CreateBotRequest};

use crate::state::AppState;

/// Create a new bot via interactive wizard or one-shot flags.
///
/// # Examples
///
/// ```bash
/// # Interactive wizard
/// bnity create bot
///
/// # One-shot with flags
/// bnity create bot --name "Luna" --description "A curious researcher"
/// ```
pub async fn create_bot(
    state: &AppState,
    name: Option<String>,
    description: Option<String>,
    category: Option<String>,
    json: bool,
) -> Result<()> {
    let name = match name {
        Some(n) => n,
        None => {
            // Interactive wizard
            Input::<String>::new()
                .with_prompt("Bot name")
                .interact_text()?
        }
    };

    let description = match description {
        Some(d) => d,
        None => {
            Input::<String>::new()
                .with_prompt("Short description")
                .default(format!("A bot named {name}"))
                .interact_text()?
        }
    };

    let category = match category {
        Some(c) => Some(c.parse::<BotCategory>().map_err(|e| anyhow::anyhow!(e))?),
        None => None,
    };

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message("Creating bot...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let request = CreateBotRequest {
        name: name.clone(),
        description: Some(description),
        category,
        tags: None,
    };

    let bot = state.bot_service.create_bot(request).await?;

    spinner.finish_and_clear();

    if json {
        println!("{}", serde_json::to_string_pretty(&bot)?);
        return Ok(());
    }

    println!();
    println!(
        "  {} Bot created successfully!",
        style("✓").green().bold()
    );
    println!();
    println!(
        "  {}  {}",
        style("Name:").bold(),
        style(&bot.name).cyan()
    );
    println!(
        "  {}  {}",
        style("Slug:").bold(),
        &bot.slug
    );
    println!(
        "  {}  {}",
        style("Status:").bold(),
        format_status(&bot.status)
    );
    println!(
        "  {}  {}",
        style("ID:").bold(),
        style(bot.id.to_string()).dim()
    );
    println!();
    println!(
        "  Files created in {}",
        style(format!("~/.boternity/bots/{}/", bot.slug)).dim()
    );
    println!(
        "    {} SOUL.md    - personality and values",
        style("•").dim()
    );
    println!(
        "    {} IDENTITY.md - model config and visual identity",
        style("•").dim()
    );
    println!(
        "    {} USER.md    - your personal briefing",
        style("•").dim()
    );
    println!();
    println!(
        "  Edit the soul: {}",
        style(format!(
            "$EDITOR ~/.boternity/bots/{}/SOUL.md",
            bot.slug
        ))
        .yellow()
    );
    println!();

    Ok(())
}

/// List all bots in a rich colored table.
pub async fn list_bots(
    state: &AppState,
    status: Option<String>,
    sort: &str,
    json: bool,
) -> Result<()> {
    use boternity_core::repository::bot::BotFilter;

    let status_filter = match status {
        Some(s) => Some(s.parse::<BotStatus>().map_err(|e| anyhow::anyhow!(e))?),
        None => None,
    };

    let filter = Some(BotFilter {
        status: status_filter,
        sort_by: Some(sort.to_string()),
        ..Default::default()
    });

    let bots = state.bot_service.list_bots(filter).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&bots)?);
        return Ok(());
    }

    if bots.is_empty() {
        println!();
        println!(
            "  {} No bots found. Create one with: {}",
            style("i").blue().bold(),
            style("bnity create bot").yellow()
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Name").fg(Color::White),
        Cell::new("Slug").fg(Color::White),
        Cell::new("Status").fg(Color::White),
        Cell::new("Description").fg(Color::White),
        Cell::new("Last Active").fg(Color::White),
    ]);

    for bot in &bots {
        let emoji = category_emoji(&bot.category);
        let name_display = format!("{} {}", emoji, bot.name);

        let status_cell = match &bot.status {
            BotStatus::Active => Cell::new("● active").fg(Color::Green),
            BotStatus::Disabled => Cell::new("○ disabled").fg(Color::Yellow),
            BotStatus::Archived => Cell::new("◌ archived").fg(Color::DarkGrey),
        };

        let last_active = match &bot.last_active_at {
            Some(dt) => format_relative_time(dt),
            None => "never".to_string(),
        };

        let desc = if bot.description.len() > 50 {
            format!("{}...", &bot.description[..47])
        } else {
            bot.description.clone()
        };

        table.add_row(vec![
            Cell::new(name_display).fg(Color::Cyan),
            Cell::new(&bot.slug).fg(Color::White),
            status_cell,
            Cell::new(desc),
            Cell::new(last_active).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!("{table}");
    println!();
    println!(
        "  {} bot{}",
        style(bots.len()).bold(),
        if bots.len() == 1 { "" } else { "s" }
    );
    println!();

    Ok(())
}

/// Show full profile for a bot.
pub async fn show_bot(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&bot)?);
        return Ok(());
    }

    let emoji = category_emoji(&bot.category);

    // Try to read SOUL.md for preview
    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");
    let soul_preview = match tokio::fs::read_to_string(&soul_path).await {
        Ok(content) => {
            // Show first few lines of the body (after frontmatter)
            let body = content
                .split("---")
                .nth(2)
                .unwrap_or("")
                .trim()
                .lines()
                .take(5)
                .collect::<Vec<_>>()
                .join("\n");
            if body.is_empty() {
                "(empty)".to_string()
            } else {
                body
            }
        }
        Err(_) => "(SOUL.md not found)".to_string(),
    };

    println!();
    println!(
        "  {} {}",
        emoji,
        style(&bot.name).cyan().bold()
    );
    println!(
        "  {}",
        style(&bot.description).dim()
    );
    println!();

    println!("  {}", style("── Details ──").dim());
    println!(
        "  {}       {}",
        style("Slug:").bold(),
        &bot.slug
    );
    println!(
        "  {}     {}",
        style("Status:").bold(),
        format_status(&bot.status)
    );
    println!(
        "  {}   {}",
        style("Category:").bold(),
        &bot.category
    );
    if !bot.tags.is_empty() {
        println!(
            "  {}       {}",
            style("Tags:").bold(),
            bot.tags.join(", ")
        );
    }
    println!(
        "  {}         {}",
        style("ID:").bold(),
        style(bot.id.to_string()).dim()
    );
    println!();

    println!("  {}", style("── Soul Preview ──").dim());
    for line in soul_preview.lines() {
        println!("  {}", line);
    }
    println!();

    println!("  {}", style("── Stats ──").dim());
    println!(
        "  {}  {}",
        style("Conversations:").bold(),
        bot.conversation_count
    );
    println!(
        "  {}   {}",
        style("Tokens used:").bold(),
        format_number(bot.total_tokens_used)
    );
    println!(
        "  {} {}",
        style("Soul versions:").bold(),
        bot.version_count
    );
    println!();

    println!("  {}", style("── Timestamps ──").dim());
    println!(
        "  {}    {}",
        style("Created:").bold(),
        bot.created_at.format("%Y-%m-%d %H:%M UTC")
    );
    println!(
        "  {}    {}",
        style("Updated:").bold(),
        bot.updated_at.format("%Y-%m-%d %H:%M UTC")
    );
    if let Some(last) = &bot.last_active_at {
        println!(
            "  {} {}",
            style("Last active:").bold(),
            last.format("%Y-%m-%d %H:%M UTC")
        );
    }
    println!();

    Ok(())
}

/// Delete a bot permanently with confirmation.
pub async fn delete_bot(
    state: &AppState,
    slug: &str,
    force: bool,
    json: bool,
) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;

    if !force && !json {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Permanently delete bot '{}' and all its data?",
                style(&bot.name).red().bold()
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.red} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Deleting {}...", bot.name));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    state.bot_service.delete_bot(&bot.id).await?;

    spinner.finish_and_clear();

    if json {
        println!(
            "{}",
            serde_json::json!({"deleted": true, "slug": slug})
        );
    } else {
        println!(
            "  {} Bot '{}' deleted.",
            style("✓").red().bold(),
            bot.name
        );
    }

    Ok(())
}

/// Clone a bot (copies soul + config, not history).
pub async fn clone_bot(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let source = state.bot_service.get_bot_by_slug(slug).await?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Cloning {}...", source.name));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let cloned = state.bot_service.clone_bot(&source.id).await?;

    spinner.finish_and_clear();

    if json {
        println!("{}", serde_json::to_string_pretty(&cloned)?);
    } else {
        println!(
            "  {} Cloned '{}' as '{}'",
            style("✓").green().bold(),
            source.name,
            style(&cloned.name).cyan()
        );
        println!(
            "  New slug: {}",
            style(&cloned.slug).bold()
        );
    }

    Ok(())
}

// --- Formatting helpers ---

fn format_status(status: &BotStatus) -> String {
    match status {
        BotStatus::Active => format!("{}", style("● active").green()),
        BotStatus::Disabled => format!("{}", style("○ disabled").yellow()),
        BotStatus::Archived => format!("{}", style("◌ archived").dim()),
    }
}

fn category_emoji(category: &BotCategory) -> &'static str {
    match category {
        BotCategory::Assistant => "🤖",
        BotCategory::Creative => "🎨",
        BotCategory::Research => "🔬",
        BotCategory::Utility => "🔧",
    }
}

fn format_relative_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now - *dt;

    if diff.num_minutes() < 1 {
        "just now".to_string()
    } else if diff.num_hours() < 1 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_days() < 1 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 30 {
        format!("{}d ago", diff.num_days())
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}

fn format_number(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
