//! Main chat loop orchestration.
//!
//! Coordinates the complete conversation lifecycle: bot resolution,
//! session creation, welcome banner, greeting, input loop with streaming
//! responses, slash commands, memory extraction, and session cleanup.
//!
//! Uses a [`FallbackChain`] for provider selection with automatic failover
//! for simple (single-agent) messages. When the LLM response contains spawn
//! instructions, the [`AgentOrchestrator`] takes over for sub-agent execution,
//! publishing events to the [`EventBus`] for real-time tree rendering.
//!
//! The `--quiet` flag suppresses sub-agent detail, showing only the final
//! synthesized response. `Ctrl+C` cancels the entire sub-agent tree.
//! `cancel N` stops an individual sub-agent by index.

use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;

use console::style;
use futures_util::StreamExt;
use tracing::{debug, info, warn};
use uuid::Uuid;

use boternity_core::agent::budget::RequestBudget;
use boternity_core::agent::context::AgentContext;
use boternity_core::agent::orchestrator::AgentOrchestrator;
use boternity_core::agent::request_context::RequestContext;
use boternity_core::agent::title::generate_title;
use boternity_core::chat::session::SessionManager;
use boternity_core::llm::health::ProviderHealth;
use boternity_core::llm::token_budget::TokenBudget;
use boternity_core::memory::box_vector::BoxVectorMemoryStore;
use boternity_core::memory::extractor::SessionMemoryExtractor;
use boternity_core::memory::store::MemoryRepository;
use boternity_infra::filesystem::identity::parse_identity_frontmatter;
use boternity_infra::filesystem::LocalFileSystem;
use boternity_infra::llm::pricing::estimate_cost;
use boternity_types::event::AgentEvent;
use boternity_types::llm::{CompletionRequest, LlmError, StreamEvent};
use boternity_types::memory::RankedMemory;

use crate::state::AppState;

use super::banner::print_welcome_banner;
use super::budget_display;
use super::commands::{self, ChatCommand};
use super::input::{ChatInput, InputEvent};
use super::renderer::ChatRenderer;
use super::tree_renderer;

/// Build a [`CompletionRequest`] from agent context and a user message.
///
/// Replicates the request building logic from `AgentEngine::build_request()`,
/// which constructs the system prompt + conversation history + user message.
fn build_completion_request(
    context: &AgentContext,
    user_message: &str,
) -> CompletionRequest {
    let mut messages = context.build_messages();

    // Add the current user message to the request
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

/// Print a failover warning to stderr with visual formatting.
fn print_failover_warning(warning: &str) {
    eprintln!(
        "  {} {}",
        style("!").yellow().bold(),
        style(warning).yellow()
    );
}

/// Print recalled memories to stderr in verbose mode.
fn print_verbose_memories(memories: &[RankedMemory]) {
    if memories.is_empty() {
        eprintln!(
            "  {} 0 memories recalled",
            style("[memory]").dim()
        );
        return;
    }

    eprintln!(
        "  {} {} memor{} recalled:",
        style("[memory]").dim(),
        memories.len(),
        if memories.len() == 1 { "y" } else { "ies" }
    );
    for mem in memories {
        let provenance_suffix = mem.provenance.as_ref().map(|p| format!(" ({p})")).unwrap_or_default();
        eprintln!(
            "    - {}: {} (score: {:.2}){}",
            style(format!("{:?}", mem.entry.category)).cyan(),
            mem.entry.fact,
            mem.relevance_score,
            style(provenance_suffix).dim()
        );
    }
}

/// Print provider chain info to stderr in verbose mode.
fn print_verbose_provider_info(provider_name: &str, provider_count: usize) {
    eprintln!(
        "  {} using {} ({} provider{} in chain)",
        style("[provider]").dim(),
        style(provider_name).cyan(),
        provider_count,
        if provider_count == 1 { "" } else { "s" }
    );
}

/// Run the interactive chat loop for a bot.
///
/// Builds a [`FallbackChain`] from the configured providers and uses it
/// for all LLM interactions. Failover events are printed to stderr.
///
/// When `verbose` is true, shows memory recall details and provider info
/// on stderr before each LLM request.
///
/// When `quiet` is true, suppresses sub-agent detail output, showing only
/// the final synthesized response.
pub async fn run_chat_loop(
    state: &AppState,
    bot_slug: &str,
    _resume_session_id: Option<String>,
    verbose: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    let bot = state.bot_service.get_bot_by_slug(bot_slug).await?;

    // Read personality files
    let soul_path = LocalFileSystem::soul_path(&state.data_dir, &bot.slug);
    let identity_path = LocalFileSystem::identity_path(&state.data_dir, &bot.slug);
    let user_path = LocalFileSystem::user_path(&state.data_dir, &bot.slug);

    let soul_content = tokio::fs::read_to_string(&soul_path).await.unwrap_or_default();
    let identity_content = tokio::fs::read_to_string(&identity_path).await.unwrap_or_default();
    let user_content = tokio::fs::read_to_string(&user_path).await.unwrap_or_default();

    // Parse identity for model config
    let identity_fm = parse_identity_frontmatter(&identity_content);
    let model = identity_fm.as_ref().map(|fm| fm.model.clone()).unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let temperature = identity_fm.as_ref().map(|fm| fm.temperature).unwrap_or(0.7);
    let max_tokens = identity_fm.as_ref().map(|fm| fm.max_tokens as u32).unwrap_or(4096);
    let bot_emoji = None::<String>;

    // Build fallback chain with all configured providers
    let mut fallback_chain = state.build_fallback_chain(&model).await?;
    let provider_count = fallback_chain.providers.len();

    // Get capabilities from the primary provider for token budget
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

    // Load memories and build agent context
    let memories = state.chat_service.load_memories(&bot.id.0).await?;
    let agent_config = boternity_types::agent::AgentConfig {
        bot_id: bot.id.0,
        bot_name: bot.name.clone(),
        bot_slug: bot.slug.clone(),
        bot_emoji: bot_emoji.clone(),
        model: model.clone(),
        temperature,
        max_tokens,
    };
    let token_budget = TokenBudget::from_capabilities(&primary_caps);
    let mut agent_context = AgentContext::new(agent_config, soul_content, identity_content.clone(), user_content, memories, token_budget);

    // Create orchestrator for sub-agent execution
    let orchestrator = AgentOrchestrator::new(3);

    // Resolve per-request token budget
    let request_budget_total = boternity_infra::config::resolve_request_budget(
        &state.global_config,
        None, // IdentityFrontmatter does not have max_request_tokens field yet
    );

    // Create session
    let session = state.chat_service.create_session(bot.id.0, model.clone()).await?;
    let mut session_manager = SessionManager::new(session);
    let session_id = session_manager.session().id;
    let session_id_str = session_id.to_string();

    // Print welcome banner
    print_welcome_banner(&bot.name, bot_emoji.as_deref(), &bot.description, &model, &session_id_str);

    if verbose {
        eprintln!(
            "  {} Verbose mode enabled. Memory recall and provider details shown on stderr.",
            style("[verbose]").dim()
        );
    }

    // Generate and display greeting using fallback chain
    let renderer = ChatRenderer::new(None);
    let greeting_spinner = indicatif::ProgressBar::new_spinner();
    greeting_spinner.set_style(indicatif::ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}").unwrap());
    greeting_spinner.set_message("thinking...");
    greeting_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let greeting_request = build_completion_request(&agent_context, "Generate a short, warm greeting message that introduces yourself and invites the user to chat. Stay fully in character. Keep it under 2 sentences.");
    let greeting = match fallback_chain.complete(&greeting_request).await {
        Ok(result) => {
            if let Some(ref warning) = result.failover_warning {
                print_failover_warning(warning);
            }
            result.response.content
        }
        Err(e) => {
            greeting_spinner.finish_and_clear();
            eprintln!("\n  {} Could not generate greeting: {e}", style("!").yellow().bold());
            "Hello! I'm ready to chat.".to_string()
        }
    };
    greeting_spinner.finish_and_clear();

    let rendered_greeting = renderer.render_final(&greeting);
    println!("  {}", rendered_greeting.trim());
    println!();

    // Persist greeting
    agent_context.add_assistant_message(greeting.clone());
    let _ = state.chat_service.save_assistant_message(session_id, greeting.clone(), model.clone(), 0, 0, "end_turn".to_string(), 0).await;

    let mut first_user_message: Option<String> = None;
    let mut first_assistant_response: Option<String> = None;

    // Prepare vector memory search components.
    // Create a fresh LanceDB connection for the chat loop's vector memory search.
    // This is cheap (just opens the existing database) and avoids ownership issues
    // with the Arc<LanceVectorMemoryStore> in AppState.
    let vector_store_for_chat = match boternity_infra::vector::lance::LanceVectorStore::new(
        state.data_dir.join("vector_store"),
    ).await {
        Ok(vs) => Some(BoxVectorMemoryStore::new(
            boternity_infra::vector::memory::LanceVectorMemoryStore::new(vs),
        )),
        Err(e) => {
            warn!(error = %e, "Failed to open vector store for memory recall; proceeding without vector search");
            None
        }
    };

    // Chat loop
    let prompt = format!("  {} ", style("You >").green().bold());
    let (mut chat_input, _writer) = ChatInput::new(prompt.clone()).map_err(|e| anyhow::anyhow!("Failed to initialize input: {e}"))?;

    loop {
        let event = chat_input.read_line().await;
        match event {
            InputEvent::Eof => {
                println!("\n  {}", style("Session ended.").dim());
                break;
            }
            InputEvent::Interrupted => {
                println!("\n  {}", style("Press Ctrl+D to exit, or keep chatting.").dim());
                continue;
            }
            InputEvent::Message(text) => {
                if text.is_empty() { continue; }

                // Slash commands
                if let Some(cmd) = commands::parse(&text) {
                    match cmd {
                        ChatCommand::Help => { commands::print_help(); continue; }
                        ChatCommand::Clear => { chat_input.clear(); continue; }
                        ChatCommand::Exit => { println!("\n  {}", style("Session ended.").dim()); break; }
                        ChatCommand::New => {
                            println!("\n  {} Starting new session is not yet implemented.", style("!").yellow().bold());
                            continue;
                        }
                        ChatCommand::History => {
                            let messages = state.chat_service.get_messages(&session_id, Some(20), None).await?;
                            println!();
                            for msg in &messages {
                                let role_label = match msg.role {
                                    boternity_types::llm::MessageRole::User => format!("{}", style("You").green()),
                                    boternity_types::llm::MessageRole::Assistant => format!("{}", style(&bot.name).cyan()),
                                    _ => "System".to_string(),
                                };
                                let preview = if msg.content.len() > 100 { format!("{}...", &msg.content[..97]) } else { msg.content.clone() };
                                println!("  {} {}", style(role_label).bold(), preview);
                            }
                            println!();
                            continue;
                        }
                        ChatCommand::Remember(fact) => {
                            let memory = boternity_types::memory::MemoryEntry {
                                id: uuid::Uuid::now_v7(), bot_id: bot.id.0, session_id,
                                fact: fact.clone(), category: boternity_types::memory::MemoryCategory::Fact,
                                importance: 4, source_message_id: None, superseded_by: None,
                                created_at: chrono::Utc::now(), is_manual: true, source_agent_id: None,
                            };
                            match state.chat_service.memory_repo().save_memory(&memory).await {
                                Ok(()) => println!("\n  {} Remembered: {}\n", style("*").cyan().bold(), style(&fact).dim()),
                                Err(e) => println!("\n  {} Failed to save memory: {e}\n", style("!").red().bold()),
                            }
                            continue;
                        }
                        ChatCommand::Unknown(cmd_name) => {
                            println!("\n  {} Unknown command: {}. Type /help for available commands.\n", style("?").yellow().bold(), style(cmd_name).dim());
                            continue;
                        }
                    }
                }

                // Send to LLM via FallbackChain
                // Note: do NOT add to agent_context here -- build_completion_request()
                // appends the user message to the request automatically.
                // We add it to history after the response completes, alongside
                // the assistant message.
                let _ = state.chat_service.save_user_message(session_id, text.clone()).await;
                if first_user_message.is_none() { first_user_message = Some(text.clone()); }

                // Vector memory recall: search for relevant memories before each request
                let recalled = if let Some(ref vs) = vector_store_for_chat {
                    state.chat_service.search_memories_for_message(
                        &bot.id.0,
                        &text,
                        &state.embedder,
                        vs,
                    ).await
                } else {
                    Vec::new()
                };

                if !recalled.is_empty() {
                    agent_context.set_recalled_memories(recalled.clone());
                }

                // Verbose: show recalled memories on stderr
                if verbose {
                    print_verbose_memories(&recalled);
                }

                // Thinking spinner
                let spinner = indicatif::ProgressBar::new_spinner();
                spinner.set_style(indicatif::ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}").unwrap());
                spinner.set_message("thinking...");
                spinner.enable_steady_tick(std::time::Duration::from_millis(80));

                // Build request and select provider via fallback chain
                let request = build_completion_request(&agent_context, &text);
                let stream_selection = match fallback_chain.select_stream(request) {
                    Ok(selection) => selection,
                    Err(e) => {
                        spinner.finish_and_clear();
                        // Handle "all providers down" clearly
                        if matches!(&e, LlmError::Provider { message } if message.contains("bnity provider status")) {
                            eprintln!("\n  {} All providers in the fallback chain are currently unavailable.", style("!").red().bold());
                            eprintln!("  {} Run {} to check provider health.", style("Tip:").dim(), style("bnity provider status").cyan());
                        } else {
                            eprintln!("\n  {} LLM error: {e}", style("!").red().bold());
                        }
                        eprintln!("  {}", style("Type a message to retry, /exit to quit.").dim());
                        continue;
                    }
                };

                let stream_provider_name = stream_selection.provider_name.clone();

                // Verbose: show provider selection on stderr
                if verbose {
                    print_verbose_provider_info(&stream_provider_name, provider_count);
                }

                // Print failover warning to stderr if we're on a non-primary provider
                if let Some(ref warning) = stream_selection.failover_warning {
                    print_failover_warning(warning);
                }

                let start_time = Instant::now();
                let mut stream = stream_selection.stream;
                let mut full_response = String::new();
                let mut input_tokens: u32 = 0;
                let mut output_tokens: u32 = 0;
                let mut stop_reason = "end_turn".to_string();
                let mut first_token_received = false;
                let mut had_error = false;
                let mut stream_error: Option<LlmError> = None;

                while let Some(event_result) = stream.next().await {
                    match event_result {
                        Ok(stream_event) => match stream_event {
                            StreamEvent::TextDelta { text: delta, .. } => {
                                if !first_token_received {
                                    spinner.finish_and_clear();
                                    first_token_received = true;
                                    print!("\n  {} ", style(&bot.name).cyan().bold());
                                    let _ = std::io::stdout().flush();
                                }
                                renderer.print_streaming_token(&delta);
                                full_response.push_str(&delta);
                            }
                            StreamEvent::Usage(usage) => {
                                input_tokens = usage.input_tokens;
                                output_tokens = usage.output_tokens;
                            }
                            StreamEvent::MessageDelta { stop_reason: sr } => { stop_reason = sr.to_string(); }
                            StreamEvent::Done => { break; }
                            _ => {}
                        },
                        Err(e) => {
                            spinner.finish_and_clear();
                            eprintln!("\n  {} LLM error: {e}", style("!").red().bold());
                            eprintln!("  {}", style("Type a message to retry, /exit to quit.").dim());
                            had_error = true;
                            // Save error for health tracking
                            if ProviderHealth::is_failover_error(&e) {
                                stream_error = Some(e);
                            }
                            break;
                        }
                    }
                }

                // Report stream outcome to fallback chain for health tracking
                if had_error {
                    if let Some(ref err) = stream_error {
                        fallback_chain.record_stream_failure(&stream_provider_name, err);
                    }
                } else {
                    fallback_chain.record_stream_success(&stream_provider_name);
                }

                if !first_token_received && !had_error { spinner.finish_and_clear(); }
                if had_error { continue; }

                // Check if the response contains spawn instructions.
                // If it does, hand off to the orchestrator for sub-agent execution.
                let spawn_instruction = boternity_core::agent::spawner::parse_spawn_instructions(&full_response);

                if let Some(_spawn_instr) = spawn_instruction {
                    // Sub-agent execution via orchestrator.
                    // Subscribe to the event bus for real-time rendering.
                    let mut event_rx = state.event_bus.subscribe();

                    // Show pre-spawn text from the initial response
                    let pre_spawn_text = boternity_core::agent::spawner::extract_text_before_spawn(&full_response);
                    if !pre_spawn_text.is_empty() {
                        println!();
                        println!("  {}", pre_spawn_text.trim());
                    }
                    println!();

                    // Create a per-request budget and context
                    let request_budget = RequestBudget::new(request_budget_total);
                    let request_ctx = RequestContext::new(Uuid::now_v7(), request_budget);

                    // Register the root cancellation token so Ctrl+C can cancel the tree.
                    // The orchestrator's RequestContext.cancellation is the root token.
                    state.agent_cancellations.insert(request_ctx.request_id, request_ctx.cancellation.clone());

                    // Create a fresh provider for the orchestrator
                    let orch_provider = match state.create_single_provider(&model).await {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("\n  {} Failed to create provider for orchestrator: {e}", style("!").red().bold());
                            continue;
                        }
                    };

                    // Clone what we need for the rendering task
                    let quiet_mode = quiet;
                    let _event_bus = state.event_bus.clone();

                    // Spawn a background task to render events from the bus.
                    // We track agent_id -> (index, total) for tree rendering.
                    let render_handle = tokio::spawn(async move {
                        let mut agent_map: HashMap<Uuid, (u8, usize, usize)> = HashMap::new();

                        loop {
                            match event_rx.recv().await {
                                Ok(event) => {
                                    if quiet_mode {
                                        // In quiet mode, only show SynthesisStarted indicator
                                        if matches!(&event, AgentEvent::SynthesisStarted { .. }) {
                                            eprintln!("  {} Synthesizing response...", style("[agents]").dim());
                                        }
                                        continue;
                                    }

                                    match &event {
                                        AgentEvent::AgentSpawned { agent_id, depth, index, total, task_description, .. } => {
                                            agent_map.insert(*agent_id, (*depth, *index, *total));
                                            println!("{}", tree_renderer::render_agent_header(*depth, *index, *total, task_description));
                                        }
                                        AgentEvent::AgentTextDelta { agent_id, text } => {
                                            if let Some(&(depth, index, total)) = agent_map.get(agent_id) {
                                                for line in text.lines() {
                                                    if !line.is_empty() {
                                                        println!("{}", tree_renderer::render_agent_text_line(depth, index, total, line));
                                                    }
                                                }
                                            }
                                        }
                                        AgentEvent::AgentCompleted { agent_id, tokens_used, duration_ms, .. } => {
                                            if let Some(&(depth, index, total)) = agent_map.get(agent_id) {
                                                println!("{}", tree_renderer::render_agent_completion(depth, index, total, *tokens_used, *duration_ms));
                                            }
                                        }
                                        AgentEvent::AgentFailed { agent_id, error, will_retry } => {
                                            let retry_note = if *will_retry { " (retrying)" } else { "" };
                                            eprintln!("  {} Agent {:?} failed: {error}{retry_note}",
                                                style("!").red().bold(),
                                                agent_map.get(agent_id).map(|(_, i, _)| i + 1).unwrap_or(0),
                                            );
                                        }
                                        AgentEvent::AgentCancelled { agent_id, reason } => {
                                            eprintln!("  {} Agent {} cancelled: {reason}",
                                                style("!").yellow().bold(),
                                                agent_map.get(agent_id).map(|(_, i, _)| i + 1).unwrap_or(0),
                                            );
                                        }
                                        AgentEvent::BudgetUpdate { tokens_used, budget_total, .. } => {
                                            // Overwrite current line with budget counter
                                            eprint!("\r{}", budget_display::render_budget_counter(*tokens_used, *budget_total));
                                        }
                                        AgentEvent::BudgetWarning { tokens_used: _, budget_total: _, .. } => {
                                            eprintln!();
                                            eprintln!("{}", budget_display::render_budget_warning_prompt());
                                            // Budget pause: the orchestrator waits for a decision via
                                            // budget_responses DashMap. In the CLI, the main input loop
                                            // can handle this. For now we auto-continue (budget pause
                                            // requires stdin reading during orchestrator execution which
                                            // is complex with rustyline-async).
                                            // TODO: wire stdin budget pause for CLI
                                        }
                                        AgentEvent::BudgetExhausted { tokens_used, budget_total, completed_agents, incomplete_agents, .. } => {
                                            eprintln!();
                                            eprintln!("{}", budget_display::render_budget_exhausted(
                                                *tokens_used, *budget_total,
                                                completed_agents.len(), incomplete_agents.len(),
                                            ));
                                        }
                                        AgentEvent::DepthLimitReached { attempted_depth, max_depth, .. } => {
                                            eprintln!("{}", tree_renderer::render_depth_limit_warning(*attempted_depth, *max_depth));
                                        }
                                        AgentEvent::CycleDetected { cycle_description, .. } => {
                                            eprintln!("{}", tree_renderer::render_cycle_warning(cycle_description));
                                        }
                                        AgentEvent::SynthesisStarted { .. } => {
                                            eprintln!();
                                            eprintln!("  {} Synthesizing final response...", style("[synthesis]").dim());
                                        }
                                        _ => {} // MemoryCreated, ProviderFailover handled elsewhere
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    debug!(skipped = n, "Event renderer lagged, some events missed");
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    break;
                                }
                            }
                        }
                    });

                    // Execute via orchestrator
                    let orch_result = orchestrator.execute(
                        &orch_provider,
                        &mut agent_context,
                        &text,
                        &request_ctx,
                        &state.event_bus,
                    ).await;

                    // Clean up cancellation token
                    state.agent_cancellations.remove(&request_ctx.request_id);

                    // Drop the render handle (it will stop when event_bus has no more events)
                    render_handle.abort();
                    let _ = render_handle.await;

                    match orch_result {
                        Ok(result) => {
                            let response_ms = start_time.elapsed().as_millis() as u64;

                            // Print the final synthesized response
                            println!();
                            print!("  {} ", style(&bot.name).cyan().bold());
                            let _ = std::io::stdout().flush();
                            let rendered = renderer.render_final(&result.final_response);
                            println!("{}", rendered.trim());
                            println!();

                            // Show completion stats with cost estimate
                            let cost = estimate_cost(
                                result.total_tokens_used / 2, // rough input/output split
                                result.total_tokens_used / 2,
                                &model,
                                &stream_provider_name,
                                &state.global_config.provider_pricing,
                            );
                            println!("{}", budget_display::render_completion_stats(
                                result.total_tokens_used,
                                request_budget_total,
                                cost,
                                response_ms as f64 / 1000.0,
                            ));
                            println!();

                            // Persist messages
                            agent_context.add_user_message(text.clone());
                            agent_context.add_assistant_message(result.final_response.clone());
                            let _ = state.chat_service.save_assistant_message(
                                session_id, result.final_response.clone(), model.clone(),
                                input_tokens + result.total_tokens_used / 2,
                                output_tokens + result.total_tokens_used / 2,
                                stop_reason.clone(), response_ms,
                            ).await;

                            // Memory extraction for sub-agents with source_agent_id tagging
                            for mem_ctx in &result.memory_contexts {
                                if let Ok(extract_provider) = state.create_single_provider(&model).await {
                                    let mem_messages = vec![
                                        boternity_types::llm::Message {
                                            role: boternity_types::llm::MessageRole::User,
                                            content: mem_ctx.task_description.clone(),
                                        },
                                        boternity_types::llm::Message {
                                            role: boternity_types::llm::MessageRole::Assistant,
                                            content: mem_ctx.response_text.clone(),
                                        },
                                    ];
                                    match SessionMemoryExtractor::extract(&extract_provider, &mem_messages, bot.id.0, session_id).await {
                                        Ok(entries) => {
                                            for mut entry in entries {
                                                entry.source_agent_id = Some(mem_ctx.agent_id);
                                                let _ = state.chat_service.memory_repo().save_memory(&entry).await;
                                            }
                                        }
                                        Err(e) => {
                                            debug!(error = %e, agent_id = %mem_ctx.agent_id, "Sub-agent memory extraction failed");
                                        }
                                    }
                                }
                            }

                            full_response = result.final_response;
                        }
                        Err(e) => {
                            let response_ms = start_time.elapsed().as_millis() as u64;
                            eprintln!("\n  {} Orchestrator error: {e}", style("!").red().bold());
                            eprintln!("  {}", style("Type a message to retry, /exit to quit.").dim());

                            // Still persist what we have
                            agent_context.add_user_message(text.clone());
                            if !full_response.is_empty() {
                                agent_context.add_assistant_message(full_response.clone());
                                let _ = state.chat_service.save_assistant_message(
                                    session_id, full_response.clone(), model.clone(),
                                    input_tokens, output_tokens, stop_reason.clone(), response_ms,
                                ).await;
                            }
                        }
                    }
                } else {
                    // Simple (no spawn) path -- use the direct streaming response
                    let response_ms = start_time.elapsed().as_millis() as u64;
                    println!();

                    // Include provider name in stats footer when using a non-primary provider
                    if stream_selection.failover_warning.is_some() {
                        renderer.print_stats_footer(output_tokens, response_ms, &format!("{} via {}", model, stream_provider_name));
                    } else {
                        renderer.print_stats_footer(output_tokens, response_ms, &model);
                    }
                    println!();

                    // Persist user + assistant messages to conversation history
                    agent_context.add_user_message(text.clone());
                    agent_context.add_assistant_message(full_response.clone());
                    let _ = state.chat_service.save_assistant_message(session_id, full_response.clone(), model.clone(), input_tokens, output_tokens, stop_reason, response_ms).await;
                }

                session_manager.add_token_usage(input_tokens, output_tokens);
                let _ = state.chat_service.update_session_tokens(&session_id, input_tokens, output_tokens).await;
                session_manager.increment_turn();

                // Title generation after first exchange
                if first_assistant_response.is_none() {
                    first_assistant_response = Some(full_response.clone());
                    if let (Some(user_msg), Some(bot_msg)) = (&first_user_message, &first_assistant_response) {
                        if let Ok(title_provider) = state.create_single_provider(&model).await {
                            match generate_title(&title_provider, user_msg, bot_msg, &model).await {
                                Ok(title) => {
                                    info!(title = %title, "Session title generated");
                                    let _ = state.chat_service.update_session_title(&session_id, title).await;
                                }
                                Err(e) => { warn!(error = %e, "Failed to generate session title"); }
                            }
                        }
                    }
                }

                // Periodic memory extraction
                if session_manager.should_extract_memory() {
                    info!(turn = session_manager.turn_count(), "Running periodic memory extraction");
                    if let Ok(extract_provider) = state.create_single_provider(&model).await {
                        let messages = agent_context.build_messages();
                        match SessionMemoryExtractor::extract(&extract_provider, &messages, bot.id.0, session_id).await {
                            Ok(entries) => {
                                for entry in entries { let _ = state.chat_service.memory_repo().save_memory(&entry).await; }
                            }
                            Err(e) => { warn!(error = %e, "Periodic memory extraction failed"); }
                        }
                    }
                }

                // Context summarization check
                if agent_context.should_summarize() {
                    info!("Context window approaching limit, summarization needed");
                }
            }
        }
    }

    // Final memory extraction
    info!("Running final memory extraction");
    let messages = agent_context.build_messages();
    if !messages.is_empty() {
        if let Ok(extract_provider) = state.create_single_provider(&model).await {
            match SessionMemoryExtractor::extract(&extract_provider, &messages, bot.id.0, session_id).await {
                Ok(entries) => {
                    let count = entries.len();
                    for entry in entries { let _ = state.chat_service.memory_repo().save_memory(&entry).await; }
                    if count > 0 { info!(count, "Memories extracted at session end"); }
                }
                Err(e) => { warn!(error = %e, "Final memory extraction failed"); }
            }
        }
    }

    let _ = state.chat_service.end_session(&session_id).await;
    session_manager.mark_completed();
    Ok(())
}
