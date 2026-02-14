//! REST API handlers for the builder system.
//!
//! Provides endpoints for the step-by-step wizard flow (create session,
//! submit answers, assemble bot, create skill) and draft management
//! (list, resume, delete). Both bot and skill creation modes are supported.
//!
//! All endpoints use the standard `ApiResponse` envelope pattern.

use std::time::Instant;

use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use boternity_core::builder::agent::{BuilderAgent, BuilderError};
use boternity_core::builder::assembler::{AssemblyResult, BotAssembler};
use boternity_core::builder::draft_store::{BuilderDraft, BuilderDraftStore};
use boternity_core::builder::memory::{BuilderMemoryEntry, BuilderMemoryStore};
use boternity_core::builder::skill_builder::{
    SkillBuildRequest, SkillBuildResult, SkillBuildType, SkillBuilder,
};
use boternity_core::builder::state::{new_builder_state, BuilderStateExt};
use boternity_types::builder::{
    BuilderAnswer, BuilderConfig, BuilderPhase, BuilderState, BuilderTurn,
};

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::http::response::ApiResponse;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Request body for creating a new builder session.
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Initial description of what the user wants to build.
    pub description: String,
    /// Session mode: "bot" (default) or "skill".
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "bot".to_string()
}

/// Response for a newly created builder session.
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub mode: String,
    pub turn: BuilderTurn,
}

/// Request body for submitting an answer.
#[derive(Debug, Deserialize)]
pub struct SubmitAnswerRequest {
    pub answer: BuilderAnswer,
}

/// Response after submitting an answer.
#[derive(Debug, Serialize)]
pub struct SubmitAnswerResponse {
    pub turn: BuilderTurn,
    pub state_summary: StateSummary,
}

/// Lightweight state summary for the client.
#[derive(Debug, Serialize)]
pub struct StateSummary {
    pub phase: BuilderPhase,
    pub question_count: usize,
}

/// Request body for assembling a bot.
#[derive(Debug, Deserialize)]
pub struct AssembleBotRequest {
    pub config: BuilderConfig,
}

/// Response after assembling a bot.
#[derive(Debug, Serialize)]
pub struct AssembleBotResponse {
    pub result: AssemblyResultDto,
}

/// Serializable assembly result DTO.
#[derive(Debug, Serialize)]
pub struct AssemblyResultDto {
    pub bot_id: String,
    pub bot_slug: String,
    pub bot_name: String,
    pub soul_path: String,
    pub identity_path: String,
    pub user_path: String,
    pub skills_attached: Vec<String>,
}

impl From<&AssemblyResult> for AssemblyResultDto {
    fn from(r: &AssemblyResult) -> Self {
        Self {
            bot_id: r.bot.id.to_string(),
            bot_slug: r.bot.slug.clone(),
            bot_name: r.bot.name.clone(),
            soul_path: r.file_paths.soul_path.display().to_string(),
            identity_path: r.file_paths.identity_path.display().to_string(),
            user_path: r.file_paths.user_path.display().to_string(),
            skills_attached: r.skills_attached.clone(),
        }
    }
}

/// Request body for creating a skill.
#[derive(Debug, Deserialize)]
pub struct CreateSkillRequest {
    pub skill_request: SkillBuildRequestDto,
}

/// Incoming skill build request DTO.
#[derive(Debug, Deserialize)]
pub struct SkillBuildRequestDto {
    pub name: String,
    pub description: String,
    /// "local" or "wasm" (defaults to "wasm").
    #[serde(default = "default_skill_type")]
    pub skill_type: String,
    pub capabilities: Option<Vec<String>>,
}

fn default_skill_type() -> String {
    "wasm".to_string()
}

/// Response after creating a skill.
#[derive(Debug, Serialize)]
pub struct CreateSkillResponse {
    pub result: SkillBuildResultDto,
}

/// Serializable skill build result DTO.
#[derive(Debug, Serialize)]
pub struct SkillBuildResultDto {
    pub name: String,
    pub description: String,
    pub skill_md_content: String,
    pub has_source_code: bool,
    pub suggested_capabilities: Vec<String>,
}

impl From<&SkillBuildResult> for SkillBuildResultDto {
    fn from(r: &SkillBuildResult) -> Self {
        Self {
            name: r.manifest.name.clone(),
            description: r.manifest.description.clone(),
            skill_md_content: r.skill_md_content.clone(),
            has_source_code: r.source_code.is_some(),
            suggested_capabilities: r
                .suggested_capabilities
                .iter()
                .map(|s| s.capability.clone())
                .collect(),
        }
    }
}

/// Response for getting a session.
#[derive(Debug, Serialize)]
pub struct GetSessionResponse {
    pub session_id: String,
    pub phase: String,
    pub initial_description: String,
    pub question_count: usize,
}

/// Response item for listing drafts.
#[derive(Debug, Serialize)]
pub struct DraftSummaryDto {
    pub session_id: String,
    pub initial_description: String,
    pub phase: String,
    pub updated_at: String,
}

/// Request body for reconfiguring a bot.
#[derive(Debug, Deserialize)]
pub struct ReconfigureBotRequest {
    pub bot_slug: String,
}

// ---------------------------------------------------------------------------
// Helper: convert BuilderError to AppError
// ---------------------------------------------------------------------------

fn builder_err(e: BuilderError) -> AppError {
    match e {
        BuilderError::LlmError(msg) => AppError::Internal(format!("Builder LLM error: {msg}")),
        BuilderError::ParseError(msg) => {
            AppError::Internal(format!("Builder parse error: {msg}"))
        }
        BuilderError::StateError(msg) => AppError::Validation(format!("Builder state: {msg}")),
        BuilderError::AssemblyError(msg) => {
            AppError::Internal(format!("Assembly error: {msg}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: create LlmBuilderAgent from AppState
// ---------------------------------------------------------------------------

use boternity_infra::builder::llm_builder::LlmBuilderAgent;
use boternity_infra::builder::sqlite_memory_store::SqliteBuilderMemoryStore;

async fn create_builder_agent(
    state: &AppState,
) -> Result<LlmBuilderAgent<SqliteBuilderMemoryStore>, AppError> {
    let model = "claude-sonnet-4-20250514";
    let provider = state
        .create_single_provider(model)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Clone the inner memory store by creating a new instance from the same pool
    let memory_store = SqliteBuilderMemoryStore::new(state.db_pool.clone());

    Ok(LlmBuilderAgent::new(
        provider,
        Some(memory_store),
        model.to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Helper: save draft from state
// ---------------------------------------------------------------------------

async fn save_draft_from_state(state: &AppState, builder_state: &BuilderState) -> Result<(), AppError> {
    let state_json = serde_json::to_string(builder_state)
        .map_err(|e| AppError::Internal(format!("Failed to serialize builder state: {e}")))?;

    let now = Utc::now();
    let draft = BuilderDraft {
        session_id: builder_state.session_id,
        state_json,
        schema_version: 1,
        created_at: now,
        updated_at: now,
    };

    state
        .builder_draft_store
        .save_draft(draft)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to save draft: {e}")))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/builder/sessions -- Create a new builder session.
///
/// Creates a session with the given mode (bot or skill), calls the builder
/// agent to start the conversation, and auto-saves the initial state as a draft.
pub async fn create_builder_session(
    State(state): State<AppState>,
    _auth: Authenticated,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let mode = match body.mode.as_str() {
        "skill" => "skill",
        _ => "bot",
    };

    let session_id = Uuid::now_v7();
    let agent = create_builder_agent(&state).await?;

    let turn = agent
        .start(session_id, &body.description)
        .await
        .map_err(builder_err)?;

    // Create initial state for draft persistence
    let builder_state = new_builder_state(session_id, body.description.clone());
    save_draft_from_state(&state, &builder_state).await?;

    let elapsed = start.elapsed().as_millis() as u64;

    let response = CreateSessionResponse {
        session_id: session_id.to_string(),
        mode: mode.to_string(),
        turn,
    };

    let resp_json = serde_json::to_value(&response).unwrap();
    let resp = ApiResponse::success(resp_json, request_id, elapsed)
        .with_link(
            "self",
            &format!("/api/v1/builder/sessions/{session_id}"),
        )
        .with_link(
            "answer",
            &format!("/api/v1/builder/sessions/{session_id}/answer"),
        );

    Ok(Json(resp))
}

/// POST /api/v1/builder/sessions/:session_id/answer -- Submit an answer.
///
/// Loads the draft state, advances the conversation with the given answer,
/// and auto-saves the updated state. Returns the next turn and a state summary.
pub async fn submit_answer(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
    Json(body): Json<SubmitAnswerRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?;

    // Load draft
    let draft = state
        .builder_draft_store
        .load_draft(&session_uuid)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to load draft: {e}")))?
        .ok_or_else(|| AppError::Validation(format!("No draft found for session {session_id}")))?;

    let mut builder_state: BuilderState = serde_json::from_str(&draft.state_json)
        .map_err(|e| AppError::Internal(format!("Failed to deserialize builder state: {e}")))?;

    // Advance conversation
    let agent = create_builder_agent(&state).await?;
    let turn = agent
        .next_turn(&mut builder_state, body.answer)
        .await
        .map_err(builder_err)?;

    // Auto-save updated draft
    save_draft_from_state(&state, &builder_state).await?;

    let elapsed = start.elapsed().as_millis() as u64;

    let response = SubmitAnswerResponse {
        turn,
        state_summary: StateSummary {
            phase: builder_state.phase.clone(),
            question_count: builder_state.question_count(),
        },
    };

    let resp_json = serde_json::to_value(&response).unwrap();
    let resp = ApiResponse::success(resp_json, request_id, elapsed);

    Ok(Json(resp))
}

/// POST /api/v1/builder/sessions/:session_id/assemble -- Assemble a bot.
///
/// Calls BotAssembler with the provided config, records builder memory,
/// and deletes the draft.
pub async fn assemble_bot(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
    Json(body): Json<AssembleBotRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?;

    // Assemble bot
    let result = BotAssembler::assemble(&*state.bot_service, &body.config)
        .await
        .map_err(builder_err)?;

    // Record builder memory
    let memory_entry = BuilderMemoryEntry {
        id: Uuid::now_v7(),
        purpose_category: serde_json::to_string(
            &boternity_core::builder::defaults::classify_purpose(&body.config.description),
        )
        .unwrap_or_default(),
        initial_description: body.config.description.clone(),
        chosen_tone: Some(body.config.personality.tone.clone()),
        chosen_model: Some(body.config.model_config.model.clone()),
        chosen_skills: result.skills_attached.clone(),
        bot_slug: Some(result.bot.slug.clone()),
        created_at: Utc::now(),
    };

    // Best effort -- don't fail the assembly if memory recording fails
    let _ = state
        .builder_memory_store
        .record_session(memory_entry)
        .await;

    // Delete draft
    let _ = state
        .builder_draft_store
        .delete_draft(&session_uuid)
        .await;

    let elapsed = start.elapsed().as_millis() as u64;

    let dto = AssemblyResultDto::from(&result);
    let response = AssembleBotResponse { result: dto };

    let resp_json = serde_json::to_value(&response).unwrap();
    let resp = ApiResponse::success(resp_json, request_id, elapsed)
        .with_link("bot", &format!("/api/v1/bots/{}", result.bot.slug));

    Ok(Json(resp))
}

/// POST /api/v1/builder/sessions/:session_id/create-skill -- Create a skill.
///
/// Generates a skill from the description, validates it, writes files to disk,
/// and deletes the draft.
pub async fn create_skill(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
    Json(body): Json<CreateSkillRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?;

    let model = "claude-sonnet-4-20250514";
    let provider = state
        .create_single_provider(model)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let skill_type = match body.skill_request.skill_type.as_str() {
        "local" => SkillBuildType::Local,
        _ => SkillBuildType::Wasm {
            language: "rust".to_string(),
        },
    };

    let build_request = SkillBuildRequest {
        name: body.skill_request.name.clone(),
        description: body.skill_request.description.clone(),
        skill_type,
        capabilities: body.skill_request.capabilities.clone(),
    };

    // Generate skill via LLM
    let result = SkillBuilder::generate_skill(&provider, &build_request)
        .await
        .map_err(builder_err)?;

    // Validate skill
    let warnings = SkillBuilder::validate_skill(&provider, &result.skill_md_content)
        .await
        .map_err(builder_err)?;

    if !warnings.is_empty() {
        tracing::warn!(
            skill = %build_request.name,
            warnings = ?warnings,
            "Skill validation warnings"
        );
    }

    // Write skill files to the skills directory
    let skills_dir = state.skills_dir();
    let skill_dir = skills_dir.join(&build_request.name);
    tokio::fs::create_dir_all(&skill_dir)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create skill directory: {e}")))?;

    // Write SKILL.md
    let skill_md_path = skill_dir.join("SKILL.md");
    tokio::fs::write(&skill_md_path, &result.skill_md_content)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write SKILL.md: {e}")))?;

    // Write source code if present
    if let Some(ref source) = result.source_code {
        let src_dir = skill_dir.join("src");
        tokio::fs::create_dir_all(&src_dir)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create src directory: {e}")))?;
        let lib_path = src_dir.join("lib.rs");
        tokio::fs::write(&lib_path, source)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write lib.rs: {e}")))?;
    }

    // Delete draft
    let _ = state
        .builder_draft_store
        .delete_draft(&session_uuid)
        .await;

    let elapsed = start.elapsed().as_millis() as u64;

    let dto = SkillBuildResultDto::from(&result);
    let response = CreateSkillResponse { result: dto };

    let resp_json = serde_json::to_value(&response).unwrap();
    let resp = ApiResponse::success(resp_json, request_id, elapsed)
        .with_link("skill", &format!("/api/v1/skills/{}", build_request.name));

    Ok(Json(resp))
}

/// GET /api/v1/builder/sessions/:session_id -- Get session state.
///
/// Returns the current state summary and phase for a draft session.
pub async fn get_session(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?;

    let draft = state
        .builder_draft_store
        .load_draft(&session_uuid)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to load draft: {e}")))?
        .ok_or_else(|| AppError::Validation(format!("No draft found for session {session_id}")))?;

    let builder_state: BuilderState = serde_json::from_str(&draft.state_json)
        .map_err(|e| AppError::Internal(format!("Failed to deserialize builder state: {e}")))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let response = GetSessionResponse {
        session_id: session_id.clone(),
        phase: format!("{:?}", builder_state.phase).to_lowercase(),
        initial_description: builder_state.initial_description.clone(),
        question_count: builder_state.question_count(),
    };

    let resp_json = serde_json::to_value(&response).unwrap();
    let resp = ApiResponse::success(resp_json, request_id, elapsed)
        .with_link("self", &format!("/api/v1/builder/sessions/{session_id}"));

    Ok(Json(resp))
}

/// GET /api/v1/builder/drafts -- List all saved drafts.
///
/// Returns lightweight summaries for the "Resume" UI.
pub async fn list_drafts(
    State(state): State<AppState>,
    _auth: Authenticated,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let summaries = state
        .builder_draft_store
        .list_drafts()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list drafts: {e}")))?;

    let dtos: Vec<DraftSummaryDto> = summaries
        .into_iter()
        .map(|s| DraftSummaryDto {
            session_id: s.session_id.to_string(),
            initial_description: s.initial_description,
            phase: s.phase,
            updated_at: s.updated_at.to_rfc3339(),
        })
        .collect();

    let elapsed = start.elapsed().as_millis() as u64;

    let dtos_json: Vec<serde_json::Value> = dtos
        .iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect();

    let resp = ApiResponse::success(dtos_json, request_id, elapsed)
        .with_link("self", "/api/v1/builder/drafts");

    Ok(Json(resp))
}

/// DELETE /api/v1/builder/sessions/:session_id -- Delete a draft.
pub async fn delete_session(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?;

    state
        .builder_draft_store
        .delete_draft(&session_uuid)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to delete draft: {e}")))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let resp = ApiResponse::success(
        serde_json::json!({"deleted": true, "session_id": session_id}),
        request_id,
        elapsed,
    );

    Ok(Json(resp))
}

/// POST /api/v1/builder/sessions/:session_id/reconfigure -- Reconfigure a bot.
///
/// Loads an existing bot's configuration, populates BuilderState, and starts
/// the reconfiguration flow.
pub async fn reconfigure_bot(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(session_id): Path<String>,
    Json(body): Json<ReconfigureBotRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let start = Instant::now();
    let request_id = Uuid::now_v7().to_string();

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?;

    // Load existing bot
    let bot = state
        .bot_service
        .get_bot_by_slug(&body.bot_slug)
        .await
        .map_err(|e| AppError::Internal(format!("Bot not found: {e}")))?;

    // Read personality files
    let _soul_content = tokio::fs::read_to_string(
        boternity_infra::filesystem::LocalFileSystem::soul_path(
            &state.data_dir,
            &bot.slug,
        ),
    )
    .await
    .unwrap_or_default();

    let identity_content = tokio::fs::read_to_string(
        boternity_infra::filesystem::LocalFileSystem::identity_path(
            &state.data_dir,
            &bot.slug,
        ),
    )
    .await
    .unwrap_or_default();

    // Parse identity for model config
    let identity_fm =
        boternity_infra::filesystem::identity::parse_identity_frontmatter(&identity_content);
    let model = identity_fm
        .as_ref()
        .map(|fm| fm.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let temperature = identity_fm.as_ref().map(|fm| fm.temperature).unwrap_or(0.7);
    let max_tokens = identity_fm
        .as_ref()
        .map(|fm| fm.max_tokens as u32)
        .unwrap_or(4096);

    // Build a config from existing bot state
    let config = BuilderConfig {
        name: bot.name.clone(),
        description: bot.description.clone(),
        category: format!("{}", bot.category),
        tags: bot.tags.clone(),
        personality: boternity_types::builder::PersonalityConfig {
            tone: "adaptive".to_string(),
            traits: vec![],
            purpose: bot.description.clone(),
            boundaries: None,
        },
        model_config: boternity_types::builder::ModelConfig {
            model,
            temperature,
            max_tokens,
        },
        skills: vec![],
    };

    // Create builder state and agent
    let mut builder_state = new_builder_state(session_uuid, bot.description.clone());
    let agent = create_builder_agent(&state).await?;

    let turn = agent
        .reconfigure(&mut builder_state, config)
        .await
        .map_err(builder_err)?;

    // Save draft
    save_draft_from_state(&state, &builder_state).await?;

    let elapsed = start.elapsed().as_millis() as u64;

    let response = CreateSessionResponse {
        session_id: session_uuid.to_string(),
        mode: "bot".to_string(),
        turn,
    };

    let resp_json = serde_json::to_value(&response).unwrap();
    let resp = ApiResponse::success(resp_json, request_id, elapsed)
        .with_link(
            "self",
            &format!("/api/v1/builder/sessions/{session_uuid}"),
        );

    Ok(Json(resp))
}
