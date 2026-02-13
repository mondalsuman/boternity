//! Session CRUD HTTP handlers.
//!
//! Endpoints:
//! - GET    /api/v1/bots/{id}/sessions     - List sessions for a bot
//! - GET    /api/v1/sessions/{id}          - Get a single session
//! - GET    /api/v1/sessions/{id}/messages - Get messages for a session
//! - DELETE /api/v1/sessions/{id}          - Delete a session
//! - POST   /api/v1/sessions/{id}/clear    - Clear messages but keep session

use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use boternity_core::chat::repository::ChatRepository;

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// Query parameters for session listing.
#[derive(Debug, Deserialize)]
pub struct SessionListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Query parameters for message listing.
#[derive(Debug, Deserialize)]
pub struct MessageListQuery {
    #[serde(default = "default_message_limit")]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

fn default_message_limit() -> Option<i64> {
    Some(100)
}

/// Resolve a bot by ID or slug (reused pattern from bot.rs).
async fn resolve_bot(
    state: &AppState,
    id_or_slug: &str,
) -> Result<boternity_types::bot::Bot, AppError> {
    match state.bot_service.get_bot_by_slug(id_or_slug).await {
        Ok(bot) => Ok(bot),
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            Ok(state.bot_service.get_bot(&id).await?)
        }
    }
}

/// Parse a UUID from a path parameter, returning a 400 error on invalid format.
fn parse_uuid(s: &str) -> Result<Uuid, AppError> {
    s.parse::<Uuid>()
        .map_err(|_| AppError::Validation(format!("Invalid UUID: {s}")))
}

/// GET /api/v1/bots/{id}/sessions - List sessions for a bot.
pub async fn list_sessions(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let sessions = state
        .chat_service
        .list_sessions(&bot.id.0, Some(query.limit), Some(query.offset))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let sessions_json: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| serde_json::to_value(s).unwrap())
        .collect();

    let resp = ApiResponse::success(sessions_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/sessions", bot.id));

    Ok(Json(resp))
}

/// GET /api/v1/sessions/{id} - Get a session by ID.
pub async fn get_session(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let sid = parse_uuid(&session_id)?;

    let session = state
        .chat_service
        .get_session(&sid)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Session not found".to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let session_json = serde_json::to_value(&session).unwrap();
    let resp = ApiResponse::success(session_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/sessions/{}", session.id))
        .with_link("messages", &format!("/api/v1/sessions/{}/messages", session.id));

    Ok(Json(resp))
}

/// GET /api/v1/sessions/{id}/messages - Get messages for a session.
pub async fn get_messages(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
    Query(query): Query<MessageListQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let sid = parse_uuid(&session_id)?;

    let messages = state
        .chat_service
        .get_messages(&sid, query.limit, query.offset)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let messages_json: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| serde_json::to_value(m).unwrap())
        .collect();

    let resp = ApiResponse::success(messages_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/sessions/{}/messages", session_id))
        .with_link("session", &format!("/api/v1/sessions/{}", session_id));

    Ok(Json(resp))
}

/// DELETE /api/v1/sessions/{id} - Delete a session and its messages.
pub async fn delete_session(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let sid = parse_uuid(&session_id)?;

    state
        .chat_service
        .chat_repo()
        .delete_session(&sid)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let resp = ApiResponse::success(
        serde_json::json!({"deleted": true}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}

/// POST /api/v1/sessions/{id}/clear - Clear messages but keep session.
pub async fn clear_session(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let sid = parse_uuid(&session_id)?;

    state
        .chat_service
        .chat_repo()
        .clear_messages(&sid)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let resp = ApiResponse::success(
        serde_json::json!({"cleared": true, "session_id": session_id}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}
