//! Workflow CRUD and execution handlers for the REST API.
//!
//! Endpoints for managing workflow definitions, triggering runs, and
//! inspecting run status with step-level logs.

use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use boternity_core::repository::workflow::WorkflowRepository;
use boternity_types::workflow::{WorkflowDefinition, WorkflowRunStatus};

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

/// Query parameters for listing workflow runs.
#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    /// Maximum number of runs to return (default 20).
    #[serde(default = "default_run_limit")]
    pub limit: u32,
}

fn default_run_limit() -> u32 {
    20
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the workflow sub-router.
///
/// Mounted at `/api/v1` by the main router. Provides CRUD for workflow
/// definitions, manual triggering, run listing, and run management.
pub fn workflow_routes() -> Router<AppState> {
    Router::new()
        // Workflow CRUD
        .route("/workflows", post(create_workflow))
        .route("/workflows", get(list_workflows))
        .route("/workflows/{id}", get(get_workflow))
        .route("/workflows/{id}", put(update_workflow))
        .route("/workflows/{id}", delete(delete_workflow))
        // Trigger and runs
        .route("/workflows/{id}/trigger", post(trigger_workflow))
        .route("/workflows/{id}/runs", get(list_runs))
        // Run management
        .route("/runs/{run_id}", get(get_run))
        .route("/runs/{run_id}/approve", post(approve_run))
        .route("/runs/{run_id}/cancel", post(cancel_run))
}

// ---------------------------------------------------------------------------
// Workflow CRUD handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/workflows - Create a new workflow definition.
pub async fn create_workflow(
    State(state): State<AppState>,
    _auth: Authenticated,
    Json(body): Json<WorkflowDefinition>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    state
        .workflow_repo
        .save_definition(&body)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let wf_json = serde_json::to_value(&body).unwrap();
    let resp = ApiResponse::success(wf_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/workflows/{}", body.id));

    Ok(Json(resp))
}

/// GET /api/v1/workflows - List all workflow definitions.
pub async fn list_workflows(
    State(state): State<AppState>,
    _auth: Authenticated,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let defs = state
        .workflow_repo
        .list_definitions(None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let defs_json: Vec<serde_json::Value> = defs
        .iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect();

    let resp = ApiResponse::success(defs_json, request_id, elapsed)
        .with_link("self", "/api/v1/workflows");

    Ok(Json(resp))
}

/// GET /api/v1/workflows/:id - Get a workflow definition by ID.
pub async fn get_workflow(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let def = state
        .workflow_repo
        .get_definition(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Workflow not found".to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let wf_json = serde_json::to_value(&def).unwrap();
    let resp = ApiResponse::success(wf_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/workflows/{}", def.id))
        .with_link("runs", &format!("/api/v1/workflows/{}/runs", def.id));

    Ok(Json(resp))
}

/// PUT /api/v1/workflows/:id - Update a workflow definition.
pub async fn update_workflow(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id): Path<Uuid>,
    Json(mut body): Json<WorkflowDefinition>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    // Ensure the ID in the path matches the body (or override)
    body.id = id;

    state
        .workflow_repo
        .save_definition(&body)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let wf_json = serde_json::to_value(&body).unwrap();
    let resp = ApiResponse::success(wf_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/workflows/{}", body.id));

    Ok(Json(resp))
}

/// DELETE /api/v1/workflows/:id - Delete a workflow definition.
pub async fn delete_workflow(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let deleted = state
        .workflow_repo
        .delete_definition(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !deleted {
        return Err(AppError::Internal("Workflow not found".to_string()));
    }

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({"deleted": true, "id": id.to_string()}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}

// ---------------------------------------------------------------------------
// Trigger and run handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/workflows/:id/trigger - Manually trigger a workflow run.
pub async fn trigger_workflow(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    // Verify the workflow exists
    let def = state
        .workflow_repo
        .get_definition(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Workflow not found".to_string()))?;

    // Create a new run record
    let run_id = Uuid::now_v7();
    let run = boternity_types::workflow::WorkflowRun {
        id: run_id,
        workflow_id: def.id,
        workflow_name: def.name.clone(),
        status: WorkflowRunStatus::Pending,
        trigger_type: "manual".to_string(),
        trigger_payload: if payload.is_null() { None } else { Some(payload) },
        context: serde_json::json!({"steps": {}}),
        started_at: chrono::Utc::now(),
        completed_at: None,
        error: None,
        concurrency_key: Some(def.name.clone()),
    };

    state
        .workflow_repo
        .create_run(&run)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let run_json = serde_json::to_value(&run).unwrap();
    let resp = ApiResponse::success(run_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/runs/{}", run_id))
        .with_link("workflow", &format!("/api/v1/workflows/{}", def.id));

    Ok(Json(resp))
}

/// GET /api/v1/workflows/:id/runs - List runs for a workflow.
pub async fn list_runs(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id): Path<Uuid>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let runs = state
        .workflow_repo
        .list_runs(&id, query.limit)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let runs_json: Vec<serde_json::Value> = runs
        .iter()
        .map(|r| serde_json::to_value(r).unwrap())
        .collect();

    let resp = ApiResponse::success(runs_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/workflows/{}/runs", id))
        .with_link("workflow", &format!("/api/v1/workflows/{}", id));

    Ok(Json(resp))
}

/// GET /api/v1/runs/:run_id - Get run detail with step logs.
pub async fn get_run(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let run = state
        .workflow_repo
        .get_run(&run_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Run not found".to_string()))?;

    // Fetch step logs for this run
    let steps = state
        .workflow_repo
        .list_step_logs(&run_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let mut run_json = serde_json::to_value(&run).unwrap();
    run_json["steps"] = serde_json::to_value(&steps).unwrap();

    let resp = ApiResponse::success(run_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/runs/{}", run_id))
        .with_link(
            "workflow",
            &format!("/api/v1/workflows/{}", run.workflow_id),
        );

    Ok(Json(resp))
}

/// POST /api/v1/runs/:run_id/approve - Approve a paused workflow run.
pub async fn approve_run(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    // Verify the run exists and is paused
    let run = state
        .workflow_repo
        .get_run(&run_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Run not found".to_string()))?;

    if run.status != WorkflowRunStatus::Paused {
        return Err(AppError::Validation(format!(
            "Run is not paused (current status: {:?})",
            run.status
        )));
    }

    // Transition to running
    state
        .workflow_repo
        .update_run_status(&run_id, WorkflowRunStatus::Running, None, None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({"run_id": run_id.to_string(), "status": "running", "approved": true}),
        request_id,
        elapsed,
    )
    .with_link("self", &format!("/api/v1/runs/{}", run_id));

    Ok(Json(resp))
}

/// POST /api/v1/runs/:run_id/cancel - Cancel a running workflow run.
pub async fn cancel_run(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    // Verify the run exists and is cancellable
    let run = state
        .workflow_repo
        .get_run(&run_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Run not found".to_string()))?;

    let cancellable = matches!(
        run.status,
        WorkflowRunStatus::Running | WorkflowRunStatus::Paused | WorkflowRunStatus::Pending
    );
    if !cancellable {
        return Err(AppError::Validation(format!(
            "Run cannot be cancelled (current status: {:?})",
            run.status
        )));
    }

    // Transition to cancelled
    state
        .workflow_repo
        .update_run_status(
            &run_id,
            WorkflowRunStatus::Cancelled,
            Some("Cancelled by user"),
            None,
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let resp = ApiResponse::success(
        serde_json::json!({"run_id": run_id.to_string(), "status": "cancelled"}),
        request_id,
        elapsed,
    )
    .with_link("self", &format!("/api/v1/runs/{}", run_id));

    Ok(Json(resp))
}
