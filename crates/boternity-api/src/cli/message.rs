//! CLI bot-to-bot messaging subcommands.
//!
//! Provides send, history, channels, subscribe, unsubscribe, and channel-history
//! operations for inter-bot communication.

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;

use boternity_core::message::envelope;
use boternity_core::repository::message::MessageRepository;
use boternity_types::message::BotSubscription;

use crate::state::AppState;

/// Bot-to-bot messaging subcommands.
#[derive(Subcommand)]
pub enum MessageCommand {
    /// Send a message from one bot to another (or to a channel).
    Send {
        /// Sender bot slug.
        #[arg(long)]
        from: String,

        /// Recipient bot slug (for direct messages).
        #[arg(long, required_unless_present = "channel")]
        to: Option<String>,

        /// Channel name (for pub/sub messages, mutually exclusive with --to).
        #[arg(long, conflicts_with = "to")]
        channel: Option<String>,

        /// Message type tag (e.g. "question", "delegation", "status_update").
        #[arg(long, default_value = "text")]
        message_type: String,

        /// Message body (JSON object or plain text).
        body: String,

        /// Wait for a reply (send-and-wait mode for direct messages).
        #[arg(long)]
        wait: bool,

        /// Timeout in seconds for send-and-wait mode.
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Show message history between two bots.
    History {
        /// First bot slug.
        bot_a: String,

        /// Second bot slug.
        bot_b: String,

        /// Maximum messages to display.
        #[arg(long, default_value = "20")]
        limit: u32,
    },

    /// List all pub/sub channels.
    Channels,

    /// Subscribe a bot to a pub/sub channel.
    Subscribe {
        /// Bot slug to subscribe.
        bot: String,

        /// Channel name to subscribe to.
        channel: String,
    },

    /// Unsubscribe a bot from a pub/sub channel.
    Unsubscribe {
        /// Bot slug to unsubscribe.
        bot: String,

        /// Channel name to unsubscribe from.
        channel: String,
    },

    /// Show message history for a channel.
    #[command(name = "channel-history")]
    ChannelHistory {
        /// Channel name.
        channel: String,

        /// Maximum messages to display.
        #[arg(long, default_value = "20")]
        limit: u32,
    },
}

/// Handle a message subcommand.
pub async fn handle_message_command(
    cmd: MessageCommand,
    state: &AppState,
    json: bool,
) -> Result<()> {
    // Lazily create the message repository from the database pool.
    let repo = boternity_infra::sqlite::message::SqliteMessageRepository::new(
        state.db_pool.clone(),
    );

    match cmd {
        MessageCommand::Send {
            from,
            to,
            channel,
            message_type,
            body,
            wait,
            timeout,
        } => {
            handle_send(
                &from,
                to.as_deref(),
                channel.as_deref(),
                &message_type,
                &body,
                wait,
                timeout,
                state,
                &repo,
                json,
            )
            .await
        }
        MessageCommand::History {
            bot_a,
            bot_b,
            limit,
        } => handle_history(&bot_a, &bot_b, limit, state, &repo, json).await,
        MessageCommand::Channels => handle_channels(&repo, json).await,
        MessageCommand::Subscribe { bot, channel } => {
            handle_subscribe(&bot, &channel, state, &repo, json).await
        }
        MessageCommand::Unsubscribe { bot, channel } => {
            handle_unsubscribe(&bot, &channel, state, &repo, json).await
        }
        MessageCommand::ChannelHistory { channel, limit } => {
            handle_channel_history(&channel, limit, &repo, json).await
        }
    }
}

// ---------------------------------------------------------------------------
// Send
// ---------------------------------------------------------------------------

async fn handle_send(
    from_slug: &str,
    to_slug: Option<&str>,
    channel_name: Option<&str>,
    message_type: &str,
    body_str: &str,
    wait: bool,
    _timeout_secs: u64,
    state: &AppState,
    repo: &impl MessageRepository,
    json: bool,
) -> Result<()> {
    // Resolve sender bot
    let sender = state
        .bot_service
        .get_bot_by_slug(from_slug)
        .await
        .with_context(|| format!("Sender bot '{from_slug}' not found"))?;

    // Parse body: try JSON first, fallback to {"text": body}
    let body: serde_json::Value = serde_json::from_str(body_str).unwrap_or_else(|_| {
        serde_json::json!({ "text": body_str })
    });

    // Build the message envelope
    let msg = if let Some(ch) = channel_name {
        // Channel (pub/sub) message
        envelope::channel(sender.id.0, &sender.name, ch, message_type, body)
    } else if let Some(to) = to_slug {
        // Direct message
        let recipient = state
            .bot_service
            .get_bot_by_slug(to)
            .await
            .with_context(|| format!("Recipient bot '{to}' not found"))?;
        envelope::direct(
            sender.id.0,
            &sender.name,
            recipient.id.0,
            message_type,
            body,
        )
    } else {
        bail!("Either --to or --channel must be specified");
    };

    let msg_id = msg.id;

    // Persist the message for audit trail
    repo.save_message(&msg)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to save message: {e}"))?;

    if json {
        let out = serde_json::json!({
            "message_id": msg_id.to_string(),
            "from": from_slug,
            "to": to_slug.or(channel_name),
            "type": message_type,
            "status": "sent",
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        let dest = if let Some(ch) = channel_name {
            format!("channel '{}'", style(ch).cyan())
        } else {
            format!("bot '{}'", style(to_slug.unwrap_or("?")).cyan())
        };

        println!();
        println!(
            "  {} Sent {} from '{}' to {}",
            style("*").green().bold(),
            style(message_type).yellow(),
            style(from_slug).cyan(),
            dest,
        );
        println!("  Message ID: {}", msg_id);

        if wait && to_slug.is_some() {
            println!(
                "  {}",
                style(format!(
                    "Note: send-and-wait requires a running message bus (bnity serve). \
                     Message was persisted but reply waiting is not available in CLI-only mode."
                ))
                .dim()
            );
        }
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

async fn handle_history(
    bot_a_slug: &str,
    bot_b_slug: &str,
    limit: u32,
    state: &AppState,
    repo: &impl MessageRepository,
    json: bool,
) -> Result<()> {
    let bot_a = state
        .bot_service
        .get_bot_by_slug(bot_a_slug)
        .await
        .with_context(|| format!("Bot '{bot_a_slug}' not found"))?;
    let bot_b = state
        .bot_service
        .get_bot_by_slug(bot_b_slug)
        .await
        .with_context(|| format!("Bot '{bot_b_slug}' not found"))?;

    let messages = repo
        .get_messages_between(&bot_a.id.0, &bot_b.id.0, limit)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get message history: {e}"))?;

    if json {
        let out: Vec<_> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id.to_string(),
                    "sender": m.sender_bot_name,
                    "type": m.message_type,
                    "body": m.body,
                    "timestamp": m.timestamp.to_rfc3339(),
                    "reply_to": m.reply_to.map(|id| id.to_string()),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if messages.is_empty() {
        println!();
        println!(
            "  No messages between '{}' and '{}'.",
            bot_a_slug, bot_b_slug
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Time").fg(Color::Cyan),
            Cell::new("Sender"),
            Cell::new("Type"),
            Cell::new("Body"),
        ]);

    for m in &messages {
        let body_preview = truncate_json_preview(&m.body, 50);
        table.add_row(vec![
            Cell::new(m.timestamp.format("%H:%M:%S").to_string()),
            Cell::new(&m.sender_bot_name),
            Cell::new(&m.message_type),
            Cell::new(body_preview),
        ]);
    }

    println!();
    println!(
        "  Messages between '{}' and '{}' (most recent first)",
        style(bot_a_slug).cyan(),
        style(bot_b_slug).cyan()
    );
    println!();
    println!("{table}");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

async fn handle_channels(repo: &impl MessageRepository, json: bool) -> Result<()> {
    let channels = repo
        .list_channels()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list channels: {e}"))?;

    if json {
        let out: Vec<_> = channels
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "created_at": c.created_at.to_rfc3339(),
                    "created_by": c.created_by_bot_id.to_string(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if channels.is_empty() {
        println!();
        println!("  No channels created yet.");
        println!(
            "  Create one by subscribing: {}",
            style("bnity message subscribe <bot> <channel>").dim()
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Channel").fg(Color::Cyan),
            Cell::new("Created At"),
            Cell::new("Created By"),
        ]);

    for c in &channels {
        table.add_row(vec![
            Cell::new(&c.name),
            Cell::new(c.created_at.format("%Y-%m-%d %H:%M").to_string()),
            Cell::new(c.created_by_bot_id.to_string().chars().take(8).collect::<String>()),
        ]);
    }

    println!();
    println!("{table}");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Subscribe
// ---------------------------------------------------------------------------

async fn handle_subscribe(
    bot_slug: &str,
    channel_name: &str,
    state: &AppState,
    repo: &impl MessageRepository,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(bot_slug)
        .await
        .with_context(|| format!("Bot '{bot_slug}' not found"))?;

    // Create the channel if it doesn't exist (auto-create on first subscribe)
    let channels = repo
        .list_channels()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list channels: {e}"))?;

    if !channels.iter().any(|c| c.name == channel_name) {
        let new_channel = boternity_types::message::Channel {
            name: channel_name.to_string(),
            created_at: chrono::Utc::now(),
            created_by_bot_id: bot.id.0,
        };
        repo.create_channel(&new_channel)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create channel: {e}"))?;
    }

    let sub = BotSubscription {
        bot_id: bot.id.0,
        channel_name: channel_name.to_string(),
        subscribed_at: chrono::Utc::now(),
    };

    repo.subscribe(&sub)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to subscribe: {e}"))?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "subscribed": true,
                "bot": bot_slug,
                "channel": channel_name,
            })
        );
    } else {
        println!();
        println!(
            "  {} Subscribed '{}' to channel '{}'",
            style("*").green().bold(),
            style(bot_slug).cyan(),
            style(channel_name).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Unsubscribe
// ---------------------------------------------------------------------------

async fn handle_unsubscribe(
    bot_slug: &str,
    channel_name: &str,
    state: &AppState,
    repo: &impl MessageRepository,
    json: bool,
) -> Result<()> {
    let bot = state
        .bot_service
        .get_bot_by_slug(bot_slug)
        .await
        .with_context(|| format!("Bot '{bot_slug}' not found"))?;

    let removed = repo
        .unsubscribe(&bot.id.0, channel_name)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to unsubscribe: {e}"))?;

    if !removed {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "unsubscribed": false,
                    "reason": "not subscribed",
                })
            );
        } else {
            println!();
            println!(
                "  Bot '{}' was not subscribed to channel '{}'.",
                bot_slug, channel_name
            );
            println!();
        }
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "unsubscribed": true,
                "bot": bot_slug,
                "channel": channel_name,
            })
        );
    } else {
        println!();
        println!(
            "  {} Unsubscribed '{}' from channel '{}'",
            style("*").green().bold(),
            style(bot_slug).cyan(),
            style(channel_name).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Channel History
// ---------------------------------------------------------------------------

async fn handle_channel_history(
    channel_name: &str,
    limit: u32,
    repo: &impl MessageRepository,
    json: bool,
) -> Result<()> {
    let messages = repo
        .get_channel_messages(channel_name, limit)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get channel history: {e}"))?;

    if json {
        let out: Vec<_> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id.to_string(),
                    "sender": m.sender_bot_name,
                    "type": m.message_type,
                    "body": m.body,
                    "timestamp": m.timestamp.to_rfc3339(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if messages.is_empty() {
        println!();
        println!(
            "  No messages in channel '{}'.",
            channel_name
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Time").fg(Color::Cyan),
            Cell::new("Sender"),
            Cell::new("Type"),
            Cell::new("Body"),
        ]);

    for m in &messages {
        let body_preview = truncate_json_preview(&m.body, 50);
        table.add_row(vec![
            Cell::new(m.timestamp.format("%H:%M:%S").to_string()),
            Cell::new(&m.sender_bot_name),
            Cell::new(&m.message_type),
            Cell::new(body_preview),
        ]);
    }

    println!();
    println!(
        "  Messages in channel '{}' (most recent first)",
        style(channel_name).cyan()
    );
    println!();
    println!("{table}");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Truncate a JSON value to a human-readable preview string.
fn truncate_json_preview(value: &serde_json::Value, max_len: usize) -> String {
    let s = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => {
            // If there's a "text" field, show that
            if let Some(text) = map.get("text") {
                if let serde_json::Value::String(t) = text {
                    return if t.len() > max_len {
                        format!("{}...", &t[..max_len])
                    } else {
                        t.clone()
                    };
                }
            }
            serde_json::to_string(value).unwrap_or_else(|_| "...".to_string())
        }
        other => serde_json::to_string(other).unwrap_or_else(|_| "...".to_string()),
    };

    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}
