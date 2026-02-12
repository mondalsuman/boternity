//! Identity and User file endpoints.
//!
//! Endpoints:
//! - GET  /api/v1/bots/{id}/identity - Read IDENTITY.md with parsed frontmatter
//! - PUT  /api/v1/bots/{id}/identity - Write IDENTITY.md content
//! - GET  /api/v1/bots/{id}/user     - Read USER.md content
//! - PUT  /api/v1/bots/{id}/user     - Write USER.md content

use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use boternity_infra::filesystem::identity::parse_identity_frontmatter;
use boternity_infra::filesystem::LocalFileSystem;

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// Request body for updating IDENTITY.md or USER.md.
#[derive(Debug, Deserialize)]
pub struct UpdateFileContent {
    /// The new file content to write.
    pub content: String,
}

/// Resolve a bot by ID or slug (reused pattern from bot.rs/soul.rs).
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

/// GET /api/v1/bots/{id}/identity - Read IDENTITY.md with parsed frontmatter.
///
/// Returns `{ raw: string, parsed?: { model, temperature, max_tokens, ... } }`.
/// If the file does not exist, returns empty raw and null parsed.
pub async fn get_identity(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;
    let identity_path = LocalFileSystem::identity_path(&state.data_dir, &bot.slug);

    let raw = tokio::fs::read_to_string(&identity_path)
        .await
        .unwrap_or_default();

    let parsed = parse_identity_frontmatter(&raw).map(|fm| {
        serde_json::json!({
            "display_name": fm.display_name,
            "category": fm.category,
            "model": fm.model,
            "provider": fm.provider,
            "temperature": fm.temperature,
            "max_tokens": fm.max_tokens,
        })
    });

    let elapsed = start.elapsed().as_millis() as u64;

    let data = serde_json::json!({
        "raw": raw,
        "parsed": parsed,
    });

    let resp = ApiResponse::success(data, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/identity", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// PUT /api/v1/bots/{id}/identity - Write IDENTITY.md content.
///
/// Writes the provided content to IDENTITY.md on disk and returns the
/// updated parsed frontmatter.
pub async fn update_identity(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<UpdateFileContent>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;
    let identity_path = LocalFileSystem::identity_path(&state.data_dir, &bot.slug);

    // Ensure parent directory exists
    if let Some(parent) = identity_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create directory: {e}")))?;
    }

    tokio::fs::write(&identity_path, &body.content)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write IDENTITY.md: {e}")))?;

    let parsed = parse_identity_frontmatter(&body.content).map(|fm| {
        serde_json::json!({
            "display_name": fm.display_name,
            "category": fm.category,
            "model": fm.model,
            "provider": fm.provider,
            "temperature": fm.temperature,
            "max_tokens": fm.max_tokens,
        })
    });

    let elapsed = start.elapsed().as_millis() as u64;

    let data = serde_json::json!({
        "raw": body.content,
        "parsed": parsed,
    });

    let resp = ApiResponse::success(data, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/identity", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// GET /api/v1/bots/{id}/user - Read USER.md content.
///
/// Returns `{ content: string }`. Empty string if file does not exist.
pub async fn get_user_context(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;
    let user_path = LocalFileSystem::user_path(&state.data_dir, &bot.slug);

    let content = tokio::fs::read_to_string(&user_path)
        .await
        .unwrap_or_default();

    let elapsed = start.elapsed().as_millis() as u64;

    let data = serde_json::json!({ "content": content });

    let resp = ApiResponse::success(data, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/user", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// PUT /api/v1/bots/{id}/user - Write USER.md content.
pub async fn update_user_context(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<UpdateFileContent>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;
    let user_path = LocalFileSystem::user_path(&state.data_dir, &bot.slug);

    // Ensure parent directory exists
    if let Some(parent) = user_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create directory: {e}")))?;
    }

    tokio::fs::write(&user_path, &body.content)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write USER.md: {e}")))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let data = serde_json::json!({ "content": body.content });

    let resp = ApiResponse::success(data, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/user", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}
