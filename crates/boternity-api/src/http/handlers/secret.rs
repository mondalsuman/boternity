//! Secret endpoint handlers for the REST API.

use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use boternity_core::service::secret::SecretService;
use boternity_types::secret::SecretScope;

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// Request body for setting a secret.
#[derive(Debug, Deserialize)]
pub struct SetSecretRequest {
    pub value: String,
}

/// GET /api/v1/secrets - List all secrets (masked).
pub async fn list_secrets(
    State(state): State<AppState>,
    _auth: Authenticated,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let entries = state
        .secret_service
        .list_secrets(&SecretScope::Global)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    // Build masked entries
    let mut secrets_json = Vec::new();
    for entry in &entries {
        let masked = match state
            .secret_service
            .get_secret(&entry.key.0, &SecretScope::Global)
            .await
        {
            Ok(Some(val)) => SecretService::mask_secret(&val),
            _ => "****".to_string(),
        };

        secrets_json.push(serde_json::json!({
            "key": entry.key.0,
            "masked_value": masked,
            "provider": entry.provider.to_string(),
            "scope": entry.scope.to_string(),
            "updated_at": entry.updated_at.to_rfc3339(),
        }));
    }

    let resp = ApiResponse::success(secrets_json, request_id, elapsed)
        .with_link("self", "/api/v1/secrets");

    Ok(Json(resp))
}

/// PUT /api/v1/secrets/:key - Set a secret value.
pub async fn set_secret(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(key): Path<String>,
    Json(body): Json<SetSecretRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    state
        .secret_service
        .set_secret(&key, &body.value, &SecretScope::Global)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let resp = ApiResponse::success(
        serde_json::json!({
            "key": key,
            "masked_value": SecretService::mask_secret(&body.value),
            "set": true,
        }),
        request_id,
        elapsed,
    )
    .with_link("self", &format!("/api/v1/secrets/{key}"));

    Ok(Json(resp))
}

/// DELETE /api/v1/secrets/:key - Delete a secret.
pub async fn delete_secret(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    state
        .secret_service
        .delete_secret(&key, &SecretScope::Global)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let resp = ApiResponse::success(
        serde_json::json!({"deleted": true, "key": key}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}
