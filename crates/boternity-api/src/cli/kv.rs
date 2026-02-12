//! Key-value store CLI subcommands for per-bot structured data.
//!
//! Provides set, get, delete, and list operations on a per-bot key-value store.
//! Values support arbitrary JSON (objects, arrays, strings, numbers, etc.).

use anyhow::{Context, Result};
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;

use boternity_core::storage::kv_store::KvStore;

use crate::state::AppState;

/// Key-value store subcommands.
#[derive(Subcommand)]
pub enum KvCommand {
    /// Set a key-value pair (value is JSON).
    Set {
        /// Bot slug.
        slug: String,

        /// Key name.
        key: String,

        /// JSON value (string, number, object, array, boolean, null).
        value: String,
    },

    /// Get a value by key.
    Get {
        /// Bot slug.
        slug: String,

        /// Key name.
        key: String,
    },

    /// Delete a key-value pair.
    Delete {
        /// Bot slug.
        slug: String,

        /// Key name.
        key: String,
    },

    /// List all keys for a bot.
    List {
        /// Bot slug.
        slug: String,
    },
}

/// Handle a KV subcommand.
pub async fn handle_kv_command(cmd: KvCommand, state: &AppState, json: bool) -> Result<()> {
    match cmd {
        KvCommand::Set { slug, key, value } => kv_set(state, &slug, &key, &value, json).await,
        KvCommand::Get { slug, key } => kv_get(state, &slug, &key, json).await,
        KvCommand::Delete { slug, key } => kv_delete(state, &slug, &key, json).await,
        KvCommand::List { slug } => kv_list(state, &slug, json).await,
    }
}

/// Set a key-value pair. Value is parsed as JSON.
///
/// If the value is not valid JSON, it is stored as a JSON string.
/// This provides a good UX: `bnity kv set bot1 name "Alice"` stores
/// the string `"Alice"`, while `bnity kv set bot1 config '{"theme":"dark"}'`
/// stores the parsed JSON object.
async fn kv_set(state: &AppState, slug: &str, key: &str, value_str: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    // Parse as JSON, fallback to string
    let value: serde_json::Value = serde_json::from_str(value_str).unwrap_or_else(|_| {
        serde_json::Value::String(value_str.to_string())
    });

    state.kv_store.set(&bot.id.0, key, &value).await?;

    if json {
        let result = serde_json::json!({
            "key": key,
            "value": value,
            "bot": slug,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!(
            "  {} Set '{}' for '{}'",
            style("ok").green(),
            style(key).cyan(),
            style(&bot.name).cyan(),
        );
        println!();
    }

    Ok(())
}

/// Get a value by key and pretty-print it.
async fn kv_get(state: &AppState, slug: &str, key: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let value = state.kv_store.get(&bot.id.0, key).await?;

    match value {
        Some(val) => {
            if json {
                let result = serde_json::json!({
                    "key": key,
                    "value": val,
                    "bot": slug,
                });
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!();
                println!(
                    "  {} = {}",
                    style(key).cyan().bold(),
                    style(serde_json::to_string_pretty(&val)?).white(),
                );
                println!();
            }
        }
        None => {
            if json {
                let result = serde_json::json!({
                    "key": key,
                    "value": null,
                    "bot": slug,
                });
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!();
                println!(
                    "  {} Key '{}' not found for '{}'",
                    style("i").blue().bold(),
                    style(key).cyan(),
                    style(&bot.name).cyan(),
                );
                println!();
            }
        }
    }

    Ok(())
}

/// Delete a key-value pair.
async fn kv_delete(state: &AppState, slug: &str, key: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    state.kv_store.delete(&bot.id.0, key).await?;

    if json {
        let result = serde_json::json!({
            "deleted": key,
            "bot": slug,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        println!(
            "  {} Deleted key '{}' from '{}'",
            style("ok").green(),
            style(key).cyan(),
            style(&bot.name).cyan(),
        );
        println!();
    }

    Ok(())
}

/// List all keys for a bot.
async fn kv_list(state: &AppState, slug: &str, json: bool) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(slug)
        .await
        .with_context(|| format!("Bot '{slug}' not found"))?;

    let keys = state.kv_store.list_keys(&bot.id.0).await?;

    if json {
        let result = serde_json::json!({
            "keys": keys,
            "count": keys.len(),
            "bot": slug,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    if keys.is_empty() {
        println!();
        println!(
            "  {} No key-value pairs for '{}'.",
            style("i").blue().bold(),
            style(&bot.name).cyan(),
        );
        println!(
            "     Set one with: bnity kv set {} <key> <json-value>",
            slug,
        );
        println!();
        return Ok(());
    }

    println!();
    println!(
        "  Keys for '{}' ({} entries)",
        style(&bot.name).cyan(),
        keys.len(),
    );
    println!();

    // Show each key with its value preview
    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Key").fg(Color::White),
        Cell::new("Value Preview").fg(Color::White),
    ]);

    for key in &keys {
        // Fetch value for preview
        let value_preview = if let Ok(Some(val)) = state.kv_store.get(&bot.id.0, key).await {
            let val_str = serde_json::to_string(&val).unwrap_or_default();
            if val_str.len() > 60 {
                format!("{}...", &val_str[..57])
            } else {
                val_str
            }
        } else {
            "(error)".to_string()
        };

        table.add_row(vec![
            Cell::new(key).fg(Color::Cyan),
            Cell::new(&value_preview).fg(Color::DarkGrey),
        ]);
    }

    println!("{table}");
    println!();

    Ok(())
}
