//! Bot CRUD handlers for the REST API.

use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::Json;

use boternity_core::repository::bot::BotFilter;
use boternity_core::repository::SortOrder;
use boternity_types::bot::{BotCategory, BotStatus, CreateBotRequest, UpdateBotRequest};

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::extractors::query::BotListQuery;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// POST /api/v1/bots - Create a new bot.
pub async fn create_bot(
    State(state): State<AppState>,
    _auth: Authenticated,
    Json(body): Json<CreateBotRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let bot = state.bot_service.create_bot(body).await?;
    let elapsed = start.elapsed().as_millis() as u64;

    let bot_json = serde_json::to_value(&bot).unwrap();
    let mut resp = ApiResponse::success(bot_json, request_id, elapsed);
    resp.links.insert("self".to_string(), format!("/api/v1/bots/{}", bot.id));
    resp.links.insert("soul".to_string(), format!("/api/v1/bots/{}/soul", bot.id));

    Ok(Json(resp))
}

/// GET /api/v1/bots - List bots with filtering and sorting.
pub async fn list_bots(
    State(state): State<AppState>,
    _auth: Authenticated,
    Query(query): Query<BotListQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let status_filter = match &query.status {
        Some(s) => Some(s.parse::<BotStatus>().map_err(|e| AppError::Validation(e))?),
        None => None,
    };

    let category_filter = match &query.category {
        Some(c) => Some(c.parse::<BotCategory>().map_err(|e| AppError::Validation(e))?),
        None => None,
    };

    let sort_order = match query.order.to_lowercase().as_str() {
        "asc" => Some(SortOrder::Asc),
        _ => Some(SortOrder::Desc),
    };

    let filter = Some(BotFilter {
        status: status_filter,
        category: category_filter,
        sort_by: Some(query.sort.clone()),
        sort_order,
        limit: query.limit,
        offset: query.offset,
    });

    let bots = state.bot_service.list_bots(filter).await?;
    let elapsed = start.elapsed().as_millis() as u64;

    let bots_json: Vec<serde_json::Value> = bots
        .iter()
        .map(|b| serde_json::to_value(b).unwrap())
        .collect();

    let resp = ApiResponse::success(bots_json, request_id, elapsed)
        .with_link("self", "/api/v1/bots");

    Ok(Json(resp))
}

/// GET /api/v1/bots/:id - Get a bot by ID or slug.
pub async fn get_bot(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    // Try by slug first, then by ID
    let bot = match state.bot_service.get_bot_by_slug(&id_or_slug).await {
        Ok(bot) => bot,
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            state.bot_service.get_bot(&id).await?
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;

    let bot_json = serde_json::to_value(&bot).unwrap();
    let resp = ApiResponse::success(bot_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}", bot.id))
        .with_link("soul", &format!("/api/v1/bots/{}/soul", bot.id))
        .with_link("secrets", "/api/v1/secrets");

    Ok(Json(resp))
}

/// PUT /api/v1/bots/:id - Update a bot.
pub async fn update_bot(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<UpdateBotRequest>,
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

    let updated = state.bot_service.update_bot(&bot.id, body).await?;
    let elapsed = start.elapsed().as_millis() as u64;

    let bot_json = serde_json::to_value(&updated).unwrap();
    let resp = ApiResponse::success(bot_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}", updated.id));

    Ok(Json(resp))
}

/// DELETE /api/v1/bots/:id - Delete a bot permanently.
pub async fn delete_bot(
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

    state.bot_service.delete_bot(&bot.id).await?;
    let elapsed = start.elapsed().as_millis() as u64;

    let resp = ApiResponse::success(
        serde_json::json!({"deleted": true, "slug": bot.slug}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}

/// POST /api/v1/bots/:id/clone - Clone a bot.
pub async fn clone_bot(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    // Resolve bot
    let source = match state.bot_service.get_bot_by_slug(&id_or_slug).await {
        Ok(bot) => bot,
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            state.bot_service.get_bot(&id).await?
        }
    };

    let cloned = state.bot_service.clone_bot(&source.id).await?;
    let elapsed = start.elapsed().as_millis() as u64;

    let bot_json = serde_json::to_value(&cloned).unwrap();
    let resp = ApiResponse::success(bot_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/bots/{}", cloned.id));

    Ok(Json(resp))
}
