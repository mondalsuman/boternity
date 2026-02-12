//! Storage CLI subcommands for managing bot file storage.
//!
//! Provides upload, download, list, info, and delete operations for bot files.
//! Text files are automatically indexed for semantic search after upload.

use anyhow::{Context, Result};
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Confirm;

use boternity_core::storage::file_store::FileStore;

use crate::state::AppState;

/// File storage subcommands.
#[derive(Subcommand)]
pub enum StorageCommand {
    /// Upload a file to a bot's storage.
    Upload {
        /// Bot slug.
        slug: String,

        /// Path to the file to upload.
        path: String,

        /// Override the stored filename (defaults to the file's basename).
        #[arg(long)]
        name: Option<String>,
    },

    /// Download a file from a bot's storage.
    Download {
        /// Bot slug.
        slug: String,

        /// Filename in the bot's storage.
        filename: String,

        /// Output path (defaults to current directory with original filename).
        #[arg(long, short)]
        output: Option<String>,
    },

    /// List all files in a bot's storage.
    List {
        /// Bot slug.
        slug: String,
    },

    /// Show detailed info about a file including version history.
    Info {
        /// Bot slug.
        slug: String,

        /// Filename to inspect.
        filename: String,
    },

    /// Delete a file from a bot's storage.
    Delete {
        /// Bot slug.
        slug: String,

        /// Filename to delete.
        filename: String,

        /// Skip confirmation prompt.
        #[arg(long)]
        force: bool,
    },
}

/// Handle a storage subcommand.
pub async fn handle_storage_command(
    cmd: StorageCommand,
    state: &AppState,
    json: bool,
) -> Result<()> {
    match cmd {
        StorageCommand::Upload { slug, path, name } => {
            upload_file(state, &slug, &path, name.as_deref(), json).await
        }
        StorageCommand::Download {
            slug,
            filename,
            output,
        } => download_file(state, &slug, &filename, output.as_deref(), json).await,
        StorageCommand::List { slug } => list_files(state, &slug, json).await,
        StorageCommand::Info { slug, filename } => file_info(state, &slug, &filename, json).await,
        StorageCommand::Delete {
            slug,
            filename,
            force,
        } => delete_file(state, &slug, &filename, force, json).await,
    }
}

/// Upload a file to a bot's storage.
///
/// Reads the file from disk, saves it to the bot's file store, and
/// automatically indexes text files for semantic search.
async fn upload_file(
    state: &AppState,
    slug: &str,
    path: &str,
    name_override: Option<&str>,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    // Read file from disk
    let file_path = std::path::Path::new(path);
    let data = tokio::fs::read(file_path)
        .await
        .with_context(|| format!("Failed to read file: {path}"))?;

    // Determine filename
    let filename = name_override.unwrap_or_else(|| {
        file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
    });

    // Save file
    let file = state.file_store.save_file(&bot.id.0, filename, &data).await?;

    // Auto-index text files
    let mime = boternity_infra::storage::detect_mime(filename);
    let indexed = if boternity_infra::storage::is_text_mime(&mime) {
        let chunks = state
            .file_indexer
            .index_file(&bot.id.0, &file.id, filename, &data)
            .await?;
        !chunks.is_empty()
    } else {
        false
    };

    if json {
        let result = serde_json::json!({
            "filename": file.filename,
            "mime_type": file.mime_type,
            "size_bytes": file.size_bytes,
            "version": file.version,
            "indexed": indexed,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!(
            "  {} Uploaded '{}' to {}",
            style("ok").green(),
            style(&file.filename).cyan(),
            style(&bot.name).cyan(),
        );
        println!(
            "     Size: {} bytes  |  MIME: {}  |  Version: {}",
            file.size_bytes, file.mime_type, file.version,
        );
        if indexed {
            println!(
                "     {} Auto-indexed for semantic search",
                style(">>").dim(),
            );
        }
        println!();
    }

    Ok(())
}

/// Download a file from a bot's storage to disk.
async fn download_file(
    state: &AppState,
    slug: &str,
    filename: &str,
    output: Option<&str>,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let data = state.file_store.get_file(&bot.id.0, filename).await?;

    let output_path = output.unwrap_or(filename);
    tokio::fs::write(output_path, &data)
        .await
        .with_context(|| format!("Failed to write to: {output_path}"))?;

    if json {
        let result = serde_json::json!({
            "filename": filename,
            "output_path": output_path,
            "size_bytes": data.len(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!(
            "  {} Downloaded '{}' -> {}",
            style("ok").green(),
            style(filename).cyan(),
            style(output_path).dim(),
        );
        println!("     {} bytes written", data.len());
        println!();
    }

    Ok(())
}

/// List all files in a bot's storage with metadata.
async fn list_files(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let files = state.file_store.list_files(&bot.id.0).await?;

    if json {
        let items: Vec<serde_json::Value> = files
            .iter()
            .map(|f| {
                serde_json::json!({
                    "filename": f.filename,
                    "mime_type": f.mime_type,
                    "size_bytes": f.size_bytes,
                    "version": f.version,
                    "is_indexed": f.is_indexed,
                    "created_at": f.created_at.to_rfc3339(),
                    "updated_at": f.updated_at.to_rfc3339(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    if files.is_empty() {
        println!();
        println!(
            "  {} No files stored for '{}'.",
            style("i").blue().bold(),
            style(&bot.name).cyan(),
        );
        println!(
            "     Upload with: bnity storage upload {} <path>",
            slug,
        );
        println!();
        return Ok(());
    }

    println!();
    println!(
        "  Files for '{}' ({} files)",
        style(&bot.name).cyan(),
        files.len(),
    );
    println!();

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Filename").fg(Color::White),
        Cell::new("MIME").fg(Color::White),
        Cell::new("Size").fg(Color::White),
        Cell::new("Version").fg(Color::White),
        Cell::new("Indexed").fg(Color::White),
        Cell::new("Updated").fg(Color::White),
    ]);

    for f in &files {
        let size_display = format_size(f.size_bytes);
        let indexed_display = if f.is_indexed { "yes" } else { "no" };
        let updated = f.updated_at.format("%Y-%m-%d %H:%M").to_string();

        table.add_row(vec![
            Cell::new(&f.filename).fg(Color::Cyan),
            Cell::new(&f.mime_type),
            Cell::new(&size_display),
            Cell::new(f.version),
            Cell::new(indexed_display),
            Cell::new(&updated).fg(Color::DarkGrey),
        ]);
    }

    println!("{table}");
    println!();

    Ok(())
}

/// Show detailed info about a file including version history.
async fn file_info(state: &AppState, slug: &str, filename: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let file = state
        .file_store
        .get_file_info(&bot.id.0, filename)
        .await
        .with_context(|| format!("File '{filename}' not found for bot '{slug}'"))?;

    let versions = state.file_store.get_versions(&file.id).await?;

    if json {
        let version_items: Vec<serde_json::Value> = versions
            .iter()
            .map(|v| {
                serde_json::json!({
                    "version": v.version,
                    "size_bytes": v.size_bytes,
                    "created_at": v.created_at.to_rfc3339(),
                })
            })
            .collect();

        let result = serde_json::json!({
            "id": file.id.to_string(),
            "filename": file.filename,
            "mime_type": file.mime_type,
            "size_bytes": file.size_bytes,
            "version": file.version,
            "is_indexed": file.is_indexed,
            "created_at": file.created_at.to_rfc3339(),
            "updated_at": file.updated_at.to_rfc3339(),
            "versions": version_items,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!();
    println!("  File: {}", style(&file.filename).cyan().bold());
    println!("  MIME: {}", file.mime_type);
    println!("  Size: {}", format_size(file.size_bytes));
    println!("  Version: {}", file.version);
    println!(
        "  Indexed: {}",
        if file.is_indexed {
            style("yes").green().to_string()
        } else {
            style("no").dim().to_string()
        }
    );
    println!("  Created: {}", file.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("  Updated: {}", file.updated_at.format("%Y-%m-%d %H:%M:%S"));

    if !versions.is_empty() {
        println!();
        println!("  Version History:");

        let mut table = Table::new();
        table.load_preset(presets::UTF8_FULL_CONDENSED);
        table.set_content_arrangement(ContentArrangement::Dynamic);

        table.set_header(vec![
            Cell::new("Version").fg(Color::White),
            Cell::new("Size").fg(Color::White),
            Cell::new("Date").fg(Color::White),
        ]);

        for v in &versions {
            let is_current = v.version == file.version;
            let version_label = if is_current {
                format!("v{} (current)", v.version)
            } else {
                format!("v{}", v.version)
            };

            table.add_row(vec![
                if is_current {
                    Cell::new(&version_label).fg(Color::Green)
                } else {
                    Cell::new(&version_label)
                },
                Cell::new(format_size(v.size_bytes)),
                Cell::new(v.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
                    .fg(Color::DarkGrey),
            ]);
        }

        println!("{table}");
    }

    println!();
    Ok(())
}

/// Delete a file from a bot's storage (removes file, versions, and index).
async fn delete_file(
    state: &AppState,
    slug: &str,
    filename: &str,
    force: bool,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    // Verify file exists before prompting
    let file = state
        .file_store
        .get_file_info(&bot.id.0, filename)
        .await
        .with_context(|| format!("File '{filename}' not found for bot '{slug}'"))?;

    if !force {
        let confirmed = Confirm::new()
            .with_prompt(format!("Delete '{filename}' from '{}'?", bot.name))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    // Deindex from vector store first (if indexed)
    if file.is_indexed {
        state
            .file_indexer
            .deindex_file(&bot.id.0, &file.id)
            .await?;
    }

    // Delete the file (removes disk files, metadata, and versions)
    state.file_store.delete_file(&bot.id.0, filename).await?;

    if json {
        let result = serde_json::json!({
            "deleted": filename,
            "bot": slug,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!(
            "  {} Deleted '{}' from '{}'",
            style("ok").green(),
            style(filename).cyan(),
            style(&bot.name).cyan(),
        );
        if file.is_indexed {
            println!(
                "     {} Removed index entries",
                style(">>").dim(),
            );
        }
        println!();
    }

    Ok(())
}

/// Format bytes into a human-readable size string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
