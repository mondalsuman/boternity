//! Soul endpoint handlers for the REST API.
//!
//! Endpoints:
//! - GET  /api/v1/bots/{id}/soul             - Get current soul
//! - PUT  /api/v1/bots/{id}/soul             - Update soul (creates new version)
//! - GET  /api/v1/bots/{id}/soul/versions    - List all versions
//! - GET  /api/v1/bots/{id}/soul/versions/{v} - Get specific version
//! - POST /api/v1/bots/{id}/soul/rollback    - Rollback to a version
//! - GET  /api/v1/bots/{id}/soul/verify      - Verify integrity
//!
//! IMPORTANT: PUT /api/v1/bots/{id}/soul is the ONLY endpoint that modifies
//! SOUL.md. This enforces the immutability contract via API.

use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// Resolve a bot by ID or slug.
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

/// GET /api/v1/bots/:id/soul - Get the current soul for a bot.
pub async fn get_soul(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let soul = state
        .soul_service
        .get_current_soul(&bot.id)
        .await
        .map_err(AppError::Soul)?
        .ok_or(AppError::Soul(boternity_types::error::SoulError::NotFound))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let soul_json = serde_json::to_value(&soul).unwrap();
    let resp = ApiResponse::success(soul_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/soul", bot.id))
        .with_link("versions", &format!("/api/v1/bots/{}/soul/versions", bot.id))
        .with_link("verify", &format!("/api/v1/bots/{}/soul/verify", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// Request body for PUT /api/v1/bots/:id/soul.
#[derive(Debug, Deserialize)]
pub struct UpdateSoulRequest {
    /// New SOUL.md content.
    pub content: String,
    /// Optional commit message.
    pub message: Option<String>,
}

/// PUT /api/v1/bots/:id/soul - Update soul content (creates a new version).
///
/// This is the ONLY API endpoint that modifies SOUL.md.
pub async fn update_soul(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<UpdateSoulRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");

    let soul = state
        .soul_service
        .update_soul(&bot.id, body.content, body.message, &soul_path)
        .await
        .map_err(AppError::Soul)?;

    let elapsed = start.elapsed().as_millis() as u64;

    let soul_json = serde_json::to_value(&soul).unwrap();
    let resp = ApiResponse::success(soul_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/soul", bot.id))
        .with_link("versions", &format!("/api/v1/bots/{}/soul/versions", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// GET /api/v1/bots/:id/soul/versions - Get soul version history.
pub async fn get_soul_versions(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let versions = state
        .soul_service
        .get_soul_versions(&bot.id)
        .await
        .map_err(AppError::Soul)?;

    let elapsed = start.elapsed().as_millis() as u64;

    let versions_json: Vec<serde_json::Value> = versions
        .iter()
        .map(|v| serde_json::to_value(v).unwrap())
        .collect();

    let resp = ApiResponse::success(versions_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/soul/versions", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// GET /api/v1/bots/:id/soul/versions/:version - Get a specific soul version.
pub async fn get_soul_version(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path((id_or_slug, version)): Path<(String, i32)>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let soul = state
        .soul_service
        .get_soul_version(&bot.id, version)
        .await
        .map_err(AppError::Soul)?
        .ok_or(AppError::Soul(boternity_types::error::SoulError::NotFound))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let soul_json = serde_json::to_value(&soul).unwrap();
    let resp = ApiResponse::success(soul_json, request_id, elapsed)
        .with_link(
            "self",
            &format!("/api/v1/bots/{}/soul/versions/{}", bot.id, version),
        )
        .with_link("versions", &format!("/api/v1/bots/{}/soul/versions", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// Request body for POST /api/v1/bots/:id/soul/rollback.
#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    /// Target version number to rollback to.
    pub version: i32,
}

/// POST /api/v1/bots/:id/soul/rollback - Rollback to a previous version.
///
/// Creates a NEW version with the old content (preserves linear history).
pub async fn rollback_soul(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<RollbackRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");

    let soul = state
        .soul_service
        .rollback_soul(&bot.id, body.version, &soul_path)
        .await
        .map_err(AppError::Soul)?;

    let elapsed = start.elapsed().as_millis() as u64;

    let soul_json = serde_json::to_value(&soul).unwrap();
    let resp = ApiResponse::success(soul_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}/soul", bot.id))
        .with_link("versions", &format!("/api/v1/bots/{}/soul/versions", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}

/// GET /api/v1/bots/:id/soul/verify - Check soul integrity.
///
/// Returns SoulIntegrityResult with hash comparison in envelope format.
pub async fn verify_soul(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = resolve_bot(&state, &id_or_slug).await?;

    let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");

    let result = state
        .soul_service
        .verify_soul_integrity(&bot.id, &soul_path)
        .await
        .map_err(AppError::Soul)?;

    let elapsed = start.elapsed().as_millis() as u64;

    let result_json = serde_json::to_value(&result).unwrap();
    let resp = ApiResponse::success(result_json, request_id, elapsed)
        .with_link("soul", &format!("/api/v1/bots/{}/soul", bot.id))
        .with_link("bot", &format!("/api/v1/bots/{}", bot.id));

    Ok(Json(resp))
}
