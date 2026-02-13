//! SSE streaming chat endpoint.
//!
//! POST /api/v1/bots/{id}/chat/stream
//!
//! Streams LLM responses as Server-Sent Events (SSE). Follows the same
//! pattern as `loop_runner.rs`: resolve bot -> read personality files ->
//! parse identity frontmatter -> build fallback chain -> build AgentContext
//! -> build CompletionRequest -> stream via FallbackChain.
//!
//! SSE event types:
//! - `session` — initial event with `{ "session_id": "..." }`
//! - `text_delta` — incremental text: `{ "text": "..." }`
//! - `usage` — token usage: `{ "input_tokens": N, "output_tokens": N }`
//! - `done` — stream complete: `{}`
//! - `error` — error occurred: `{ "message": "..." }`

use std::convert::Infallible;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_stream::Stream;

use boternity_core::agent::context::AgentContext;
use boternity_core::llm::health::ProviderHealth;
use boternity_core::llm::token_budget::TokenBudget;
use boternity_core::memory::box_vector::BoxVectorMemoryStore;
use boternity_infra::filesystem::identity::parse_identity_frontmatter;
use boternity_infra::filesystem::LocalFileSystem;
use boternity_types::llm::{CompletionRequest, StreamEvent};

use crate::http::error::AppError;
use crate::http::extractors::auth::Authenticated;
use crate::state::AppState;

/// Request body for the streaming chat endpoint.
#[derive(Debug, Deserialize)]
pub struct StreamChatRequest {
    /// Existing session ID to continue; if absent, a new session is created.
    pub session_id: Option<String>,
    /// The user message to send to the bot.
    pub message: String,
}

/// Build a [`CompletionRequest`] from agent context and a user message.
///
/// Replicates the request building logic from `AgentEngine::build_request()`.
fn build_completion_request(context: &AgentContext, user_message: &str) -> CompletionRequest {
    let mut messages = context.build_messages();

    messages.push(boternity_types::llm::Message {
        role: boternity_types::llm::MessageRole::User,
        content: user_message.to_string(),
    });

    CompletionRequest {
        model: context.agent_config.model.clone(),
        messages,
        system: Some(context.system_prompt.clone()),
        max_tokens: context.agent_config.max_tokens,
        temperature: Some(context.agent_config.temperature),
        stream: true,
        stop_sequences: None,
    }
}

/// POST /api/v1/bots/{id}/chat/stream — SSE streaming chat.
///
/// Resolves the bot, builds the LLM context, streams the response as SSE
/// events, and persists both user and assistant messages after completion.
pub async fn stream_chat(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<StreamChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Resolve bot by slug or ID
    let bot = match state.bot_service.get_bot_by_slug(&id_or_slug).await {
        Ok(bot) => bot,
        Err(_) => {
            let id = id_or_slug
                .parse()
                .map_err(|_| AppError::Bot(boternity_types::error::BotError::NotFound))?;
            state.bot_service.get_bot(&id).await?
        }
    };

    // Reject chat for non-active bots
    if bot.status != boternity_types::bot::BotStatus::Active {
        return Err(AppError::Validation(format!(
            "Bot '{}' is {} and cannot chat",
            bot.name, bot.status
        )));
    }

    // Read personality files
    let soul_content = tokio::fs::read_to_string(LocalFileSystem::soul_path(
        &state.data_dir,
        &bot.slug,
    ))
    .await
    .unwrap_or_default();

    let identity_content = tokio::fs::read_to_string(LocalFileSystem::identity_path(
        &state.data_dir,
        &bot.slug,
    ))
    .await
    .unwrap_or_default();

    let user_content = tokio::fs::read_to_string(LocalFileSystem::user_path(
        &state.data_dir,
        &bot.slug,
    ))
    .await
    .unwrap_or_default();

    // Parse identity frontmatter for model config
    let identity_fm = parse_identity_frontmatter(&identity_content);
    let model = identity_fm
        .as_ref()
        .map(|fm| fm.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let temperature = identity_fm.as_ref().map(|fm| fm.temperature).unwrap_or(0.7);
    let max_tokens = identity_fm
        .as_ref()
        .map(|fm| fm.max_tokens as u32)
        .unwrap_or(4096);

    // Build fallback chain
    let mut fallback_chain = state
        .build_fallback_chain(&model)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Get capabilities for token budget
    let primary_caps = fallback_chain
        .providers
        .first()
        .map(|(_, p)| p.capabilities().clone())
        .unwrap_or_else(|| boternity_types::llm::ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false,
            extended_thinking: false,
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
        });

    // Resolve or create session
    let session_id = if let Some(ref sid) = body.session_id {
        sid.parse::<uuid::Uuid>()
            .map_err(|_| AppError::Validation("Invalid session_id format".to_string()))?
    } else {
        let session = state
            .chat_service
            .create_session(bot.id.0, model.clone())
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        session.id
    };

    // Load memories and build agent context
    let memories = state
        .chat_service
        .load_memories(&bot.id.0)
        .await
        .unwrap_or_default();
    let agent_config = boternity_types::agent::AgentConfig {
        bot_id: bot.id.0,
        bot_name: bot.name.clone(),
        bot_slug: bot.slug.clone(),
        bot_emoji: None,
        model: model.clone(),
        temperature,
        max_tokens,
    };
    let token_budget = TokenBudget::from_capabilities(&primary_caps);
    let mut agent_context = AgentContext::new(
        agent_config,
        soul_content,
        identity_content,
        user_content,
        memories,
        token_budget,
    );

    // Load conversation history into agent context for session continuation
    let history = state
        .chat_service
        .get_messages(&session_id, None, None)
        .await
        .unwrap_or_default();
    for msg in &history {
        match msg.role {
            boternity_types::chat::MessageRole::User => {
                agent_context.add_user_message(msg.content.clone());
            }
            boternity_types::chat::MessageRole::Assistant => {
                agent_context.add_assistant_message(msg.content.clone());
            }
            _ => {}
        }
    }

    // Vector memory recall
    let vector_store_for_chat =
        match boternity_infra::vector::lance::LanceVectorStore::new(
            state.data_dir.join("vector_store"),
        )
        .await
        {
            Ok(vs) => Some(BoxVectorMemoryStore::new(
                boternity_infra::vector::memory::LanceVectorMemoryStore::new(vs),
            )),
            Err(_) => None,
        };

    let recalled = if let Some(ref vs) = vector_store_for_chat {
        state
            .chat_service
            .search_memories_for_message(&bot.id.0, &body.message, &state.embedder, vs)
            .await
    } else {
        Vec::new()
    };

    if !recalled.is_empty() {
        agent_context.set_recalled_memories(recalled);
    }

    // Build the completion request
    let request = build_completion_request(&agent_context, &body.message);

    // Select provider and get stream
    let stream_selection = fallback_chain
        .select_stream(request)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let provider_name = stream_selection.provider_name.clone();
    let llm_stream = stream_selection.stream;

    // Capture values needed in the async stream closure
    let user_message = body.message.clone();
    let chat_service = state.chat_service.clone();
    let model_for_save = model.clone();
    let bot_service = state.bot_service.clone();
    let bot_id = bot.id.clone();

    // Build the SSE stream
    let sse_stream = async_stream::stream! {
        // Emit session event
        let session_json = serde_json::json!({ "session_id": session_id.to_string() });
        yield Ok::<_, Infallible>(Event::default().event("session").data(session_json.to_string()));

        let start_time = Instant::now();
        let mut full_response = String::new();
        let mut input_tokens: u32 = 0;
        let mut output_tokens: u32 = 0;
        let mut stop_reason = "end_turn".to_string();
        let mut had_error = false;
        let mut stream_error_is_failover = false;

        let mut llm_stream = std::pin::pin!(llm_stream);

        while let Some(event_result) = llm_stream.next().await {
            match event_result {
                Ok(stream_event) => match stream_event {
                    StreamEvent::TextDelta { text: delta, .. } => {
                        let data = serde_json::json!({ "text": delta });
                        yield Ok(Event::default().event("text_delta").data(data.to_string()));
                        full_response.push_str(&delta);
                    }
                    StreamEvent::Usage(usage) => {
                        input_tokens = usage.input_tokens;
                        output_tokens = usage.output_tokens;
                        let data = serde_json::to_string(&usage).unwrap_or_default();
                        yield Ok(Event::default().event("usage").data(data));
                    }
                    StreamEvent::MessageDelta { stop_reason: sr } => {
                        stop_reason = sr.to_string();
                    }
                    StreamEvent::Done => {
                        break;
                    }
                    _ => {}
                },
                Err(e) => {
                    let data = serde_json::json!({ "message": e.to_string() });
                    yield Ok(Event::default().event("error").data(data.to_string()));
                    had_error = true;
                    stream_error_is_failover = ProviderHealth::is_failover_error(&e);
                    break;
                }
            }
        }

        if !had_error && !full_response.is_empty() {
            let response_ms = start_time.elapsed().as_millis() as u64;

            // Persist messages
            let _ = chat_service
                .save_user_message(session_id, user_message)
                .await;
            let _ = chat_service
                .save_assistant_message(
                    session_id,
                    full_response,
                    model_for_save,
                    input_tokens,
                    output_tokens,
                    stop_reason,
                    response_ms,
                )
                .await;

            // Update session token counts
            let _ = chat_service
                .update_session_tokens(&session_id, input_tokens, output_tokens)
                .await;

            // Update bot's last_active_at timestamp
            let _ = bot_service.touch_activity(&bot_id).await;
        }

        // Emit done event
        yield Ok(Event::default().event("done").data("{}"));

        // Suppress unused variable warnings
        let _ = provider_name;
        let _ = stream_error_is_failover;
    };

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
