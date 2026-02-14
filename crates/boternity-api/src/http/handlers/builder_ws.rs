//! WebSocket handler for the Forge chat builder.
//!
//! `/ws/builder/:session_id` enables real-time builder conversations over
//! WebSocket for both bot creation and standalone skill creation modes.
//!
//! Follows the Phase 5 WebSocket pattern: `tokio::select!` single-loop
//! with ping/pong heartbeat. The builder agent drives the multi-turn
//! conversation; drafts are auto-saved on every turn.
//!
//! Message protocol uses serde-tagged JSON enums (`WsBuilderMessage` for
//! incoming, `WsBuilderResponse` for outgoing), consistent with the
//! Phase 5 `WsCommand` convention.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use boternity_core::builder::agent::BuilderAgent;
use boternity_core::builder::assembler::{AssemblyResult, BotAssembler};
use boternity_core::builder::draft_store::{BuilderDraft, BuilderDraftStore};
use boternity_core::builder::memory::{BuilderMemoryEntry, BuilderMemoryStore};
use boternity_core::builder::skill_builder::{
    SkillBuildRequest, SkillBuildResult, SkillBuildType, SkillBuilder,
};
use boternity_core::builder::state::new_builder_state;
use boternity_types::builder::{
    BuilderAnswer, BuilderConfig, BuilderState, BuilderTurn,
};

use boternity_infra::builder::llm_builder::LlmBuilderAgent;
use boternity_infra::builder::sqlite_memory_store::SqliteBuilderMemoryStore;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// WebSocket message types
// ---------------------------------------------------------------------------

/// Incoming message from a WebSocket client.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsBuilderMessage {
    /// Start a new bot creation session.
    StartBot { description: String },
    /// Start a new standalone skill creation session.
    StartSkill { description: String },
    /// Submit an answer (works for both bot and skill modes).
    Answer { answer: BuilderAnswer },
    /// Confirm bot assembly.
    AssembleBot { config: BuilderConfig },
    /// Confirm skill creation.
    CreateSkill { skill_request: SkillRequestDto },
    /// Resume from a saved draft.
    Resume,
    /// Keep-alive ping.
    Ping,
}

/// Skill request DTO for WebSocket messages.
#[derive(Debug, Deserialize)]
struct SkillRequestDto {
    name: String,
    description: String,
    #[serde(default = "default_skill_type")]
    skill_type: String,
    capabilities: Option<Vec<String>>,
}

fn default_skill_type() -> String {
    "wasm".to_string()
}

/// Outgoing response to a WebSocket client.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsBuilderResponse {
    /// Next builder turn (works for both modes).
    Turn { turn: BuilderTurn },
    /// Bot was successfully assembled.
    BotAssembled { result: AssemblyResultDto },
    /// Skill was successfully created.
    SkillCreated { result: SkillBuildResultDto },
    /// An error occurred.
    Error { message: String },
    /// Pong response.
    Pong,
}

/// Serializable assembly result for WebSocket.
#[derive(Debug, Serialize)]
struct AssemblyResultDto {
    bot_id: String,
    bot_slug: String,
    bot_name: String,
    skills_attached: Vec<String>,
}

impl From<&AssemblyResult> for AssemblyResultDto {
    fn from(r: &AssemblyResult) -> Self {
        Self {
            bot_id: r.bot.id.to_string(),
            bot_slug: r.bot.slug.clone(),
            bot_name: r.bot.name.clone(),
            skills_attached: r.skills_attached.clone(),
        }
    }
}

/// Serializable skill build result for WebSocket.
#[derive(Debug, Serialize)]
struct SkillBuildResultDto {
    name: String,
    description: String,
    has_source_code: bool,
    suggested_capabilities: Vec<String>,
}

impl From<&SkillBuildResult> for SkillBuildResultDto {
    fn from(r: &SkillBuildResult) -> Self {
        Self {
            name: r.manifest.name.clone(),
            description: r.manifest.description.clone(),
            has_source_code: r.source_code.is_some(),
            suggested_capabilities: r
                .suggested_capabilities
                .iter()
                .map(|s| s.capability.clone())
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Session mode tracking
// ---------------------------------------------------------------------------

/// Which mode the session is operating in.
#[derive(Debug, Clone, PartialEq)]
enum SessionMode {
    Bot,
    Skill,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// GET /ws/builder/:session_id -- Upgrade to WebSocket for builder chat.
pub async fn builder_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_builder_ws(socket, session_id, state))
}

/// Core WebSocket connection handler for the builder.
///
/// Uses `tokio::select!` single-loop pattern (per 05-06 decision).
/// Manages builder state locally and auto-saves drafts on every turn.
async fn handle_builder_ws(socket: WebSocket, session_id_str: String, state: AppState) {
    let session_id = match Uuid::parse_str(&session_id_str) {
        Ok(id) => id,
        Err(_) => {
            tracing::warn!(
                session_id = %session_id_str,
                "Invalid session_id in WebSocket URL"
            );
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Heartbeat interval (30s, consistent with Phase 5)
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));

    // Session state -- initialized on first StartBot/StartSkill/Resume message
    let mut builder_state: Option<BuilderState> = None;
    let mut session_mode: Option<SessionMode> = None;

    // Check if a draft exists and pre-load state
    if let Ok(Some(draft)) = state.builder_draft_store.load_draft(&session_id).await {
        if let Ok(loaded_state) = serde_json::from_str::<BuilderState>(&draft.state_json) {
            builder_state = Some(loaded_state);
            // Default to bot mode for existing drafts (mode will be set on
            // StartBot/StartSkill if the client sends one instead of Resume)
            session_mode = Some(SessionMode::Bot);
            tracing::debug!(
                %session_id,
                "Loaded existing draft for WebSocket builder session"
            );
        }
    }

    loop {
        tokio::select! {
            // --- Branch 1: Heartbeat ping ---
            _ = heartbeat.tick() => {
                let pong = serde_json::to_string(&WsBuilderResponse::Pong).unwrap();
                if ws_sender.send(Message::Text(pong.into())).await.is_err() {
                    break;
                }
            }

            // --- Branch 2: Incoming WebSocket messages ---
            msg_result = ws_receiver.next() => {
                match msg_result {
                    Some(Ok(Message::Text(text))) => {
                        let response = process_builder_message(
                            &text,
                            session_id,
                            &state,
                            &mut builder_state,
                            &mut session_mode,
                        ).await;

                        if let Some(resp) = response {
                            let json = serde_json::to_string(&resp).unwrap();
                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        // Client disconnected -- draft remains saved (resumable)
                        break;
                    }
                    Some(Err(err)) => {
                        tracing::debug!("Builder WebSocket receive error: {err}");
                        break;
                    }
                    // Ignore binary, ping, pong protocol frames
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    tracing::debug!(%session_id, "Builder WebSocket connection closed");
}

// ---------------------------------------------------------------------------
// Message processing
// ---------------------------------------------------------------------------

/// Process a single incoming WebSocket message and return a response.
async fn process_builder_message(
    text: &str,
    session_id: Uuid,
    state: &AppState,
    builder_state: &mut Option<BuilderState>,
    session_mode: &mut Option<SessionMode>,
) -> Option<WsBuilderResponse> {
    let msg: WsBuilderMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(err) => {
            tracing::warn!(
                raw = %text,
                error = %err,
                "Ignoring malformed builder WebSocket message"
            );
            return Some(WsBuilderResponse::Error {
                message: format!("Invalid message: {err}"),
            });
        }
    };

    match msg {
        WsBuilderMessage::StartBot { description } => {
            handle_start(session_id, &description, SessionMode::Bot, state, builder_state, session_mode).await
        }
        WsBuilderMessage::StartSkill { description } => {
            handle_start(session_id, &description, SessionMode::Skill, state, builder_state, session_mode).await
        }
        WsBuilderMessage::Answer { answer } => {
            handle_answer(state, builder_state, answer).await
        }
        WsBuilderMessage::AssembleBot { config } => {
            handle_assemble_bot(session_id, state, builder_state, session_mode, &config).await
        }
        WsBuilderMessage::CreateSkill { skill_request } => {
            handle_create_skill(session_id, state, builder_state, session_mode, skill_request).await
        }
        WsBuilderMessage::Resume => {
            handle_resume(session_id, state, builder_state, session_mode).await
        }
        WsBuilderMessage::Ping => {
            Some(WsBuilderResponse::Pong)
        }
    }
}

// ---------------------------------------------------------------------------
// Individual message handlers
// ---------------------------------------------------------------------------

/// Handle StartBot or StartSkill messages.
async fn handle_start(
    session_id: Uuid,
    description: &str,
    mode: SessionMode,
    state: &AppState,
    builder_state: &mut Option<BuilderState>,
    session_mode: &mut Option<SessionMode>,
) -> Option<WsBuilderResponse> {
    let agent = match create_builder_agent(state).await {
        Ok(a) => a,
        Err(e) => return Some(WsBuilderResponse::Error { message: e }),
    };

    match agent.start(session_id, description).await {
        Ok(turn) => {
            let new_state = new_builder_state(session_id, description.to_string());
            // Save draft
            let _ = save_draft(state, &new_state).await;
            *builder_state = Some(new_state);
            *session_mode = Some(mode);
            Some(WsBuilderResponse::Turn { turn })
        }
        Err(e) => Some(WsBuilderResponse::Error {
            message: format!("Failed to start builder: {e}"),
        }),
    }
}

/// Handle Answer messages.
async fn handle_answer(
    state: &AppState,
    builder_state: &mut Option<BuilderState>,
    answer: BuilderAnswer,
) -> Option<WsBuilderResponse> {
    let bs = match builder_state.as_mut() {
        Some(s) => s,
        None => {
            return Some(WsBuilderResponse::Error {
                message: "No active session. Send StartBot or StartSkill first.".to_string(),
            });
        }
    };

    let agent = match create_builder_agent(state).await {
        Ok(a) => a,
        Err(e) => return Some(WsBuilderResponse::Error { message: e }),
    };

    match agent.next_turn(bs, answer).await {
        Ok(turn) => {
            // Auto-save draft on every turn
            let _ = save_draft(state, bs).await;
            Some(WsBuilderResponse::Turn { turn })
        }
        Err(e) => Some(WsBuilderResponse::Error {
            message: format!("Builder error: {e}"),
        }),
    }
}

/// Handle AssembleBot messages (only valid in Bot mode).
async fn handle_assemble_bot(
    session_id: Uuid,
    state: &AppState,
    builder_state: &mut Option<BuilderState>,
    session_mode: &mut Option<SessionMode>,
    config: &BuilderConfig,
) -> Option<WsBuilderResponse> {
    // Validate mode
    if session_mode.as_ref() != Some(&SessionMode::Bot) {
        return Some(WsBuilderResponse::Error {
            message: "AssembleBot is only valid in bot creation mode.".to_string(),
        });
    }

    match BotAssembler::assemble(&*state.bot_service, config).await {
        Ok(result) => {
            // Record builder memory (best effort)
            if let Some(bs) = builder_state.as_ref() {
                let memory_entry = BuilderMemoryEntry {
                    id: Uuid::now_v7(),
                    purpose_category: bs
                        .purpose_category
                        .as_ref()
                        .and_then(|c| serde_json::to_string(c).ok())
                        .unwrap_or_default(),
                    initial_description: bs.initial_description.clone(),
                    chosen_tone: Some(config.personality.tone.clone()),
                    chosen_model: Some(config.model_config.model.clone()),
                    chosen_skills: result.skills_attached.clone(),
                    bot_slug: Some(result.bot.slug.clone()),
                    created_at: Utc::now(),
                };
                let _ = state.builder_memory_store.record_session(memory_entry).await;
            }

            // Delete draft
            let _ = state.builder_draft_store.delete_draft(&session_id).await;

            let dto = AssemblyResultDto::from(&result);
            Some(WsBuilderResponse::BotAssembled { result: dto })
        }
        Err(e) => Some(WsBuilderResponse::Error {
            message: format!("Assembly failed: {e}"),
        }),
    }
}

/// Handle CreateSkill messages (only valid in Skill mode).
async fn handle_create_skill(
    session_id: Uuid,
    state: &AppState,
    builder_state: &mut Option<BuilderState>,
    session_mode: &mut Option<SessionMode>,
    skill_request: SkillRequestDto,
) -> Option<WsBuilderResponse> {
    // Validate mode
    if session_mode.as_ref() != Some(&SessionMode::Skill) {
        return Some(WsBuilderResponse::Error {
            message: "CreateSkill is only valid in skill creation mode.".to_string(),
        });
    }

    let model = "claude-sonnet-4-20250514";
    let provider = match state.create_single_provider(model).await {
        Ok(p) => p,
        Err(e) => {
            return Some(WsBuilderResponse::Error {
                message: format!("Failed to create provider: {e}"),
            });
        }
    };

    let skill_type = match skill_request.skill_type.as_str() {
        "local" => SkillBuildType::Local,
        _ => SkillBuildType::Wasm {
            language: "rust".to_string(),
        },
    };

    let build_request = SkillBuildRequest {
        name: skill_request.name.clone(),
        description: skill_request.description.clone(),
        skill_type,
        capabilities: skill_request.capabilities.clone(),
    };

    // Generate skill via LLM
    let result = match SkillBuilder::generate_skill(&provider, &build_request).await {
        Ok(r) => r,
        Err(e) => {
            return Some(WsBuilderResponse::Error {
                message: format!("Skill generation failed: {e}"),
            });
        }
    };

    // Validate (non-blocking warnings only)
    if let Ok(warnings) = SkillBuilder::validate_skill(&provider, &result.skill_md_content).await {
        if !warnings.is_empty() {
            tracing::warn!(
                skill = %build_request.name,
                warnings = ?warnings,
                "Skill validation warnings (WebSocket)"
            );
        }
    }

    // Write skill files to disk
    let skills_dir = state.skills_dir();
    let skill_dir = skills_dir.join(&build_request.name);
    if let Err(e) = tokio::fs::create_dir_all(&skill_dir).await {
        return Some(WsBuilderResponse::Error {
            message: format!("Failed to create skill directory: {e}"),
        });
    }

    if let Err(e) = tokio::fs::write(skill_dir.join("SKILL.md"), &result.skill_md_content).await {
        return Some(WsBuilderResponse::Error {
            message: format!("Failed to write SKILL.md: {e}"),
        });
    }

    if let Some(ref source) = result.source_code {
        let src_dir = skill_dir.join("src");
        let _ = tokio::fs::create_dir_all(&src_dir).await;
        if let Err(e) = tokio::fs::write(src_dir.join("lib.rs"), source).await {
            return Some(WsBuilderResponse::Error {
                message: format!("Failed to write lib.rs: {e}"),
            });
        }
    }

    // Delete draft
    let _ = state.builder_draft_store.delete_draft(&session_id).await;

    let _ = builder_state; // Suppress unused warning

    let dto = SkillBuildResultDto::from(&result);
    Some(WsBuilderResponse::SkillCreated { result: dto })
}

/// Handle Resume messages.
async fn handle_resume(
    session_id: Uuid,
    state: &AppState,
    builder_state: &mut Option<BuilderState>,
    session_mode: &mut Option<SessionMode>,
) -> Option<WsBuilderResponse> {
    // Load draft if not already loaded
    if builder_state.is_none() {
        match state.builder_draft_store.load_draft(&session_id).await {
            Ok(Some(draft)) => {
                match serde_json::from_str::<BuilderState>(&draft.state_json) {
                    Ok(loaded) => {
                        *builder_state = Some(loaded);
                        *session_mode = Some(SessionMode::Bot); // Default; client can override
                    }
                    Err(e) => {
                        return Some(WsBuilderResponse::Error {
                            message: format!("Failed to deserialize draft: {e}"),
                        });
                    }
                }
            }
            Ok(None) => {
                return Some(WsBuilderResponse::Error {
                    message: "No draft found to resume.".to_string(),
                });
            }
            Err(e) => {
                return Some(WsBuilderResponse::Error {
                    message: format!("Failed to load draft: {e}"),
                });
            }
        }
    }

    let bs = builder_state.as_ref().unwrap();

    let agent = match create_builder_agent(state).await {
        Ok(a) => a,
        Err(e) => return Some(WsBuilderResponse::Error { message: e }),
    };

    match agent.resume(bs).await {
        Ok(turn) => Some(WsBuilderResponse::Turn { turn }),
        Err(e) => Some(WsBuilderResponse::Error {
            message: format!("Resume failed: {e}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a builder agent from AppState.
async fn create_builder_agent(
    state: &AppState,
) -> Result<LlmBuilderAgent<SqliteBuilderMemoryStore>, String> {
    let model = "claude-sonnet-4-20250514";
    let provider = state
        .create_single_provider(model)
        .await
        .map_err(|e| format!("Failed to create LLM provider: {e}"))?;

    let memory_store = SqliteBuilderMemoryStore::new(state.db_pool.clone());

    Ok(LlmBuilderAgent::new(
        provider,
        Some(memory_store),
        model.to_string(),
    ))
}

/// Save builder state as a draft.
async fn save_draft(state: &AppState, builder_state: &BuilderState) -> Result<(), String> {
    let state_json = serde_json::to_string(builder_state)
        .map_err(|e| format!("Failed to serialize builder state: {e}"))?;

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
        .map_err(|e| format!("Failed to save draft: {e}"))?;

    Ok(())
}
