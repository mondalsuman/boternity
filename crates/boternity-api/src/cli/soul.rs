//! Soul management CLI commands: edit, history, diff, rollback, verify.
//!
//! These commands provide the explicit admin interface for managing a bot's
//! SOUL.md file. All modifications go through SoulService::update_soul(),
//! enforcing the immutability invariant.

use anyhow::Result;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Confirm;

use crate::state::AppState;

/// Open a bot's SOUL.md in $EDITOR for editing.
///
/// If the content changes after the editor closes, calls
/// SoulService::update_soul() to create a new versioned entry.
///
/// # Examples
///
/// ```bash
/// bnity soul edit luna
/// ```
pub async fn edit_soul(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;
    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");

    // Read current content
    let current_content = tokio::fs::read_to_string(&soul_path).await?;

    // Determine editor
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "nano".to_string()
            } else {
                "vi".to_string()
            }
        });

    // Write current content to a temp file for editing
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join("SOUL.md");
    tokio::fs::write(&temp_path, &current_content).await?;

    // Open editor
    let status = std::process::Command::new(&editor)
        .arg(&temp_path)
        .status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    // Read edited content
    let new_content = tokio::fs::read_to_string(&temp_path).await?;

    if new_content == current_content {
        if json {
            println!("{}", serde_json::json!({"changed": false}));
        } else {
            println!("  No changes made.");
        }
        return Ok(());
    }

    // Update soul through the versioned path
    let soul = state
        .soul_service
        .update_soul(&bot.id, new_content, None, &soul_path)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&soul)?);
    } else {
        let short_hash = &soul.hash[..8.min(soul.hash.len())];
        println!(
            "  {} Soul updated: version {} (hash: {})",
            style("ok").green().bold(),
            style(soul.version).cyan(),
            style(short_hash).dim()
        );
    }

    Ok(())
}

/// Display the version history of a bot's soul.
///
/// Shows a table with version number, hash (first 8 chars), relative time,
/// and commit message. Most recent version at top.
///
/// # Examples
///
/// ```bash
/// bnity soul history luna
/// ```
pub async fn soul_history(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;

    let versions = state.soul_service.get_soul_versions(&bot.id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&versions)?);
        return Ok(());
    }

    if versions.is_empty() {
        println!();
        println!(
            "  {} No soul versions found for '{}'",
            style("i").blue().bold(),
            slug
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Version").fg(Color::White),
        Cell::new("Hash").fg(Color::White),
        Cell::new("Created").fg(Color::White),
        Cell::new("Message").fg(Color::White),
    ]);

    // Show most recent first
    for v in versions.iter().rev() {
        let short_hash = &v.hash[..8.min(v.hash.len())];
        let relative = format_relative_time(&v.created_at);
        let message = v.message.as_deref().unwrap_or("-");

        table.add_row(vec![
            Cell::new(format!("v{}", v.version)).fg(Color::Cyan),
            Cell::new(short_hash).fg(Color::DarkGrey),
            Cell::new(&relative).fg(Color::DarkGrey),
            Cell::new(message),
        ]);
    }

    println!();
    println!(
        "  Soul history for '{}'",
        style(&bot.name).cyan().bold()
    );
    println!();
    println!("{table}");
    println!();
    println!(
        "  {} version{}",
        style(versions.len()).bold(),
        if versions.len() == 1 { "" } else { "s" }
    );
    println!();

    Ok(())
}

/// Show a line-by-line diff between two soul versions.
///
/// Defaults: compares current version with the previous one.
///
/// # Examples
///
/// ```bash
/// bnity soul diff luna
/// bnity soul diff luna --from 1 --to 3
/// ```
pub async fn soul_diff(
    state: &AppState,
    slug: &str,
    from: Option<i32>,
    to: Option<i32>,
    json: bool,
) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;

    // Determine current version
    let current = state
        .soul_service
        .get_current_soul(&bot.id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No soul found for bot '{slug}'"))?;

    let to_version = to.unwrap_or(current.version);
    let from_version = from.unwrap_or_else(|| (to_version - 1).max(1));

    if from_version == to_version {
        if json {
            println!(
                "{}",
                serde_json::json!({"from": from_version, "to": to_version, "diff": ""})
            );
        } else {
            println!("  No difference (same version).");
        }
        return Ok(());
    }

    let diff = state
        .soul_service
        .get_soul_diff(&bot.id, from_version, to_version)
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "from": from_version,
                "to": to_version,
                "diff": diff,
            })
        );
        return Ok(());
    }

    println!();
    println!(
        "  Diff: v{} -> v{} for '{}'",
        style(from_version).cyan(),
        style(to_version).cyan(),
        style(&bot.name).bold()
    );
    println!();

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix('+') {
            println!("  {}", style(format!("+{rest}")).green());
        } else if let Some(rest) = line.strip_prefix('-') {
            println!("  {}", style(format!("-{rest}")).red());
        } else {
            println!("  {line}");
        }
    }

    println!();

    Ok(())
}

/// Rollback a bot's soul to a previous version.
///
/// Creates a NEW version with the old content, preserving linear history.
/// Prompts for confirmation unless --force is used.
///
/// # Examples
///
/// ```bash
/// bnity soul rollback luna 1
/// ```
pub async fn soul_rollback(
    state: &AppState,
    slug: &str,
    version: i32,
    force: bool,
    json: bool,
) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;

    if !force && !json {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Rollback {}'s soul to version {}? This creates a new version.",
                style(&bot.name).cyan().bold(),
                style(version).cyan()
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");

    let new_soul = state
        .soul_service
        .rollback_soul(&bot.id, version, &soul_path)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&new_soul)?);
    } else {
        println!(
            "  {} Soul rolled back: now at version {} (content from version {})",
            style("ok").green().bold(),
            style(new_soul.version).cyan(),
            style(version).cyan()
        );
    }

    Ok(())
}

/// Verify the integrity of a bot's SOUL.md file.
///
/// Computes the SHA-256 hash of the file on disk and compares it against
/// the stored hash. Reports valid or INTEGRITY VIOLATION.
///
/// # Examples
///
/// ```bash
/// bnity soul verify luna
/// ```
pub async fn soul_verify(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state.bot_service.get_bot_by_slug(slug).await?;
    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");

    let result = state
        .soul_service
        .verify_soul_integrity(&bot.id, &soul_path)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    if result.valid {
        let short_hash = &result.expected_hash[..8.min(result.expected_hash.len())];
        println!(
            "  {} Soul integrity verified (version {}, hash: {})",
            style("ok").green().bold(),
            style(result.version).cyan(),
            style(short_hash).dim()
        );
    } else {
        let short_expected = &result.expected_hash[..8.min(result.expected_hash.len())];
        let short_actual = &result.actual_hash[..8.min(result.actual_hash.len())];
        println!();
        println!(
            "  {} SOUL INTEGRITY VIOLATION!",
            style("ERROR").red().bold()
        );
        println!();
        println!(
            "  Expected hash: {}",
            style(short_expected).green()
        );
        println!(
            "  Actual hash:   {}",
            style(short_actual).red()
        );
        println!();
        println!(
            "  The SOUL.md file has been modified outside of Boternity."
        );
        println!(
            "  Use `{}` to view versions",
            style(format!("bnity soul history {slug}")).yellow()
        );
        println!(
            "  Use `{}` to properly update",
            style(format!("bnity soul edit {slug}")).yellow()
        );
        println!();
    }

    Ok(())
}

// --- Helper ---

fn format_relative_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now - *dt;

    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 30 {
        format!("{}d ago", diff.num_days())
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}
