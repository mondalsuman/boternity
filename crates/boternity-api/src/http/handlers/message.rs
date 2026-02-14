//! Bot-to-bot messaging handlers for the REST API.
//!
//! Endpoints for sending messages between bots, viewing message history,
//! managing pub/sub channels, and subscribing/unsubscribing bots.

use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use boternity_core::repository::message::MessageRepository;
use boternity_types::message::{BotMessage, BotSubscription, MessageRecipient};

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / query types
// ---------------------------------------------------------------------------

/// Request body for sending a message.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    /// Sender bot ID.
    pub sender_bot_id: Uuid,
    /// Sender bot name (denormalized).
    pub sender_bot_name: String,
    /// Recipient specification (direct or channel).
    pub recipient: MessageRecipient,
    /// Message type tag (e.g. "question", "delegation").
    pub message_type: String,
    /// Message body (flexible JSON).
    pub body: serde_json::Value,
    /// Optional reply-to message ID.
    #[serde(default)]
    pub reply_to: Option<Uuid>,
}

/// Query parameters for message history with pagination.
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// Maximum messages to return (default 50).
    #[serde(default = "default_history_limit")]
    pub limit: u32,
}

fn default_history_limit() -> u32 {
    50
}

/// Request body for subscribing a bot to a channel.
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    /// The bot ID to subscribe.
    pub bot_id: Uuid,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/messages/send - Send a bot-to-bot message.
pub async fn send_message(
    State(state): State<AppState>,
    _auth: Authenticated,
    Json(body): Json<SendMessageRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let msg = BotMessage {
        id: Uuid::now_v7(),
        sender_bot_id: body.sender_bot_id,
        sender_bot_name: body.sender_bot_name,
        recipient: body.recipient,
        message_type: body.message_type,
        body: body.body,
        timestamp: chrono::Utc::now(),
        reply_to: body.reply_to,
    };

    state
        .message_repo
        .save_message(&msg)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let msg_json = serde_json::to_value(&msg).unwrap();
    let resp = ApiResponse::success(msg_json, request_id, elapsed);

    Ok(Json(resp))
}

/// GET /api/v1/messages/history/:bot_a/:bot_b - Get direct message history between two bots.
pub async fn get_message_history(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path((bot_a, bot_b)): Path<(Uuid, Uuid)>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let messages = state
        .message_repo
        .get_messages_between(&bot_a, &bot_b, query.limit)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let msgs_json: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| serde_json::to_value(m).unwrap())
        .collect();

    let resp = ApiResponse::success(msgs_json, request_id, elapsed).with_link(
        "self",
        &format!("/api/v1/messages/history/{}/{}", bot_a, bot_b),
    );

    Ok(Json(resp))
}

/// GET /api/v1/channels - List all pub/sub channels.
pub async fn list_channels(
    State(state): State<AppState>,
    _auth: Authenticated,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let channels = state
        .message_repo
        .list_channels()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let channels_json: Vec<serde_json::Value> = channels
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();

    let resp = ApiResponse::success(channels_json, request_id, elapsed)
        .with_link("self", "/api/v1/channels");

    Ok(Json(resp))
}

/// POST /api/v1/channels/:name/subscribe - Subscribe a bot to a channel.
pub async fn subscribe_to_channel(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(name): Path<String>,
    Json(body): Json<SubscribeRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let sub = BotSubscription {
        bot_id: body.bot_id,
        channel_name: name.clone(),
        subscribed_at: chrono::Utc::now(),
    };

    state
        .message_repo
        .subscribe(&sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let sub_json = serde_json::to_value(&sub).unwrap();
    let resp = ApiResponse::success(sub_json, request_id, elapsed);

    Ok(Json(resp))
}

/// DELETE /api/v1/channels/:name/subscribe/:bot_id - Unsubscribe a bot from a channel.
pub async fn unsubscribe_from_channel(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path((name, bot_id)): Path<(String, Uuid)>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let removed = state
        .message_repo
        .unsubscribe(&bot_id, &name)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({"unsubscribed": removed, "channel": name, "bot_id": bot_id.to_string()}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}

/// GET /api/v1/channels/:name/messages - Get messages for a channel.
pub async fn get_channel_messages(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(name): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let messages = state
        .message_repo
        .get_channel_messages(&name, query.limit)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let msgs_json: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| serde_json::to_value(m).unwrap())
        .collect();

    let resp = ApiResponse::success(msgs_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/channels/{}/messages", name));

    Ok(Json(resp))
}
