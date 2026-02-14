//! Webhook receiver handler for the REST API.
//!
//! Receives incoming webhook requests, verifies authentication
//! (HMAC-SHA256 or bearer token) via the `WebhookRegistry`, and
//! spawns a background workflow execution via `DagExecutor`.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use boternity_core::repository::workflow::WorkflowRepository;
use boternity_core::workflow::executor::WorkflowExecutor;

use crate::http::error::AppError;
use crate::http::response::ApiResponse;
use crate::state::AppState;

/// POST /api/v1/webhooks/:path - Receive an incoming webhook.
///
/// Looks up the webhook path in the `WebhookRegistry`, verifies the
/// request authentication, then spawns a background workflow execution
/// via `DagExecutor`. The executor creates its own `WorkflowRun` record.
///
/// Authentication is determined by the webhook registration:
/// - **HMAC-SHA256**: Reads `X-Hub-Signature-256` header
/// - **Bearer token**: Reads `Authorization` header
/// - **None**: No authentication required
pub async fn receive_webhook(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();
    let webhook_path = format!("/{}", path);

    // Extract auth headers
    let signature_header = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    // Verify request against the webhook registry
    let config = state
        .webhook_registry
        .verify_request(&webhook_path, &body, signature_header, auth_header)
        .map_err(|e| match &e {
            boternity_infra::workflow::webhook_handler::WebhookError::PathNotFound(_) => {
                AppError::Internal(format!("Webhook not found: {webhook_path}"))
            }
            boternity_infra::workflow::webhook_handler::WebhookError::HmacVerificationFailed
            | boternity_infra::workflow::webhook_handler::WebhookError::BearerVerificationFailed => {
                AppError::Unauthorized("Webhook authentication failed".to_string())
            }
            boternity_infra::workflow::webhook_handler::WebhookError::MissingAuth(msg) => {
                AppError::Unauthorized(msg.clone())
            }
            _ => AppError::Internal(e.to_string()),
        })?;

    // Parse the body as JSON (best-effort; raw bytes become null if not valid JSON)
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);

    // Verify the workflow still exists
    let def = state
        .workflow_repo
        .get_definition(&config.workflow_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| {
            AppError::Internal(format!(
                "Workflow {} no longer exists",
                config.workflow_id
            ))
        })?;

    // Prepare trigger payload
    let trigger_payload = if payload.is_null() { None } else { Some(payload) };

    tracing::info!(
        webhook_path = %webhook_path,
        workflow_id = %def.id,
        "Webhook triggering workflow execution"
    );

    // Spawn background execution task -- the executor creates its own run record
    let executor = Arc::clone(&state.workflow_executor);
    let def_clone = def.clone();
    let trigger_payload_clone = trigger_payload.clone();
    tokio::spawn(async move {
        match executor
            .execute(&def_clone, "webhook", trigger_payload_clone)
            .await
        {
            Ok(result) => {
                tracing::info!(
                    run_id = %result.run_id,
                    status = ?result.status,
                    steps = result.completed_steps.len(),
                    "webhook-triggered workflow execution completed"
                );
            }
            Err(e) => {
                tracing::error!(
                    workflow_id = %def_clone.id,
                    error = %e,
                    "webhook-triggered workflow execution failed"
                );
            }
        }
    });

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({
            "workflow_id": def.id.to_string(),
            "workflow_name": def.name,
            "status": "submitted",
            "trigger": "webhook",
        }),
        request_id,
        elapsed,
    )
    .with_link("runs", &format!("/api/v1/workflows/{}/runs", def.id))
    .with_link("workflow", &format!("/api/v1/workflows/{}", def.id));

    Ok(Json(resp))
}
