//! Dashboard statistics endpoint.
//!
//! GET /api/v1/stats - Aggregate counts for the fleet dashboard.

use std::time::Instant;

use axum::extract::State;
use axum::Json;
use sqlx::Row;

use boternity_core::chat::repository::ChatRepository;

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// GET /api/v1/stats - Aggregate dashboard statistics.
///
/// Returns bot counts by status, total sessions, active sessions, and
/// total messages. Uses efficient COUNT(*) SQL queries directly on the
/// database pool for performance.
pub async fn get_stats(
    State(state): State<AppState>,
    _auth: Authenticated,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    // Bot counts by status (efficient single query with conditional counts)
    let bot_row = sqlx::query(
        r#"SELECT
            COUNT(*) as total_bots,
            SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END) as active_bots,
            SUM(CASE WHEN status = 'disabled' THEN 1 ELSE 0 END) as disabled_bots,
            SUM(CASE WHEN status = 'archived' THEN 1 ELSE 0 END) as archived_bots
        FROM bots"#,
    )
    .fetch_one(&state.db_pool.reader)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to query bot stats: {e}")))?;

    let total_bots: i64 = bot_row.try_get("total_bots").unwrap_or(0);
    let active_bots: i64 = bot_row.try_get("active_bots").unwrap_or(0);
    let disabled_bots: i64 = bot_row.try_get("disabled_bots").unwrap_or(0);
    let archived_bots: i64 = bot_row.try_get("archived_bots").unwrap_or(0);

    // Session counts
    let total_sessions = state
        .chat_service
        .chat_repo()
        .count_sessions()
        .await
        .unwrap_or(0);

    // Active sessions (status = 'active')
    let active_session_row =
        sqlx::query("SELECT COUNT(*) as cnt FROM chat_sessions WHERE status = 'active'")
            .fetch_one(&state.db_pool.reader)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to query active sessions: {e}")))?;
    let active_sessions: i64 = active_session_row.try_get("cnt").unwrap_or(0);

    // Total messages
    let total_messages = state
        .chat_service
        .chat_repo()
        .count_messages()
        .await
        .unwrap_or(0);

    let elapsed = start.elapsed().as_millis() as u64;

    let data = serde_json::json!({
        "total_bots": total_bots,
        "active_bots": active_bots,
        "disabled_bots": disabled_bots,
        "archived_bots": archived_bots,
        "total_sessions": total_sessions,
        "active_sessions": active_sessions,
        "total_messages": total_messages,
    });

    let resp = ApiResponse::success(data, request_id, elapsed)
        .with_link("self", "/api/v1/stats")
        .with_link("bots", "/api/v1/bots");

    Ok(Json(resp))
}
