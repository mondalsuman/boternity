//! Soul endpoint handlers for the REST API.

use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// GET /api/v1/bots/:id/soul - Get the current soul for a bot.
pub async fn get_soul(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    // Resolve bot
    let bot = match state.bot_service.get_bot_by_slug(&id_or_slug).await {
        Ok(bot) => bot,
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            state.bot_service.get_bot(&id).await?
        }
    };

    let soul = state
        .soul_service
        .get_current_soul(&bot.id)
        .await
        .map_err(|e| AppError::Soul(e))?
        .ok_or(AppError::Soul(boternity_types::error::SoulError::NotFound))?;

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

    // Resolve bot
    let bot = match state.bot_service.get_bot_by_slug(&id_or_slug).await {
        Ok(bot) => bot,
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            state.bot_service.get_bot(&id).await?
        }
    };

    let versions = state
        .soul_service
        .get_soul_versions(&bot.id)
        .await
        .map_err(|e| AppError::Soul(e))?;

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
