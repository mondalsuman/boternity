//! Main chat loop orchestration.
//!
//! Coordinates the complete conversation lifecycle: bot resolution,
//! session creation, welcome banner, greeting, input loop with streaming
//! responses, slash commands, memory extraction, and session cleanup.

use std::io::Write;
use std::time::Instant;

use console::style;
use futures_util::StreamExt;
use secrecy::SecretString;
use tracing::{info, warn};

use boternity_core::agent::context::AgentContext;
use boternity_core::agent::engine::AgentEngine;
use boternity_core::agent::title::generate_title;
use boternity_core::chat::session::SessionManager;
use boternity_core::llm::box_provider::BoxLlmProvider;
use boternity_core::llm::token_budget::TokenBudget;
use boternity_core::memory::extractor::SessionMemoryExtractor;
use boternity_core::memory::store::MemoryRepository;
use boternity_infra::filesystem::identity::parse_identity_frontmatter;
use boternity_infra::filesystem::LocalFileSystem;
use boternity_infra::llm::anthropic::AnthropicProvider;
use boternity_types::llm::StreamEvent;
use boternity_types::secret::SecretScope;

use crate::state::AppState;

use super::banner::print_welcome_banner;
use super::commands::{self, ChatCommand};
use super::input::{ChatInput, InputEvent};
use super::renderer::ChatRenderer;

/// Create a BoxLlmProvider from the state's secret service.
async fn create_provider(state: &AppState, model: &str) -> anyhow::Result<BoxLlmProvider> {
    let api_key_value = state
        .secret_service
        .get_secret("ANTHROPIC_API_KEY", &SecretScope::Global)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "ANTHROPIC_API_KEY not found. Set it with: bnity set secret ANTHROPIC_API_KEY"
            )
        })?;
    let api_key = SecretString::from(api_key_value);
    let anthropic = AnthropicProvider::new(api_key, model.to_string());
    Ok(BoxLlmProvider::new(anthropic))
}

/// Run the interactive chat loop for a bot.
pub async fn run_chat_loop(
    state: &AppState,
    bot_slug: &str,
    _resume_session_id: Option<String>,
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

    // Create LLM provider
    let provider = create_provider(state, &model).await?;

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
    let token_budget = TokenBudget::from_capabilities(provider.capabilities());
    let mut agent_context = AgentContext::new(agent_config, soul_content, identity_content.clone(), user_content, memories, token_budget);
    let agent_engine = AgentEngine::new(provider);

    // Create session
    let session = state.chat_service.create_session(bot.id.0, model.clone()).await?;
    let mut session_manager = SessionManager::new(session);
    let session_id = session_manager.session().id;
    let session_id_str = session_id.to_string();

    // Print welcome banner
    print_welcome_banner(&bot.name, bot_emoji.as_deref(), &bot.description, &model, &session_id_str);

    // Generate and display greeting
    let renderer = ChatRenderer::new(None);
    let greeting_spinner = indicatif::ProgressBar::new_spinner();
    greeting_spinner.set_style(indicatif::ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}").unwrap());
    greeting_spinner.set_message("thinking...");
    greeting_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let greeting = match agent_engine.generate_greeting(&agent_context).await {
        Ok(g) => g,
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
                                created_at: chrono::Utc::now(), is_manual: true,
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

                // Send to LLM
                agent_context.add_user_message(text.clone());
                let _ = state.chat_service.save_user_message(session_id, text.clone()).await;
                if first_user_message.is_none() { first_user_message = Some(text.clone()); }

                // Thinking spinner
                let spinner = indicatif::ProgressBar::new_spinner();
                spinner.set_style(indicatif::ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}").unwrap());
                spinner.set_message("thinking...");
                spinner.enable_steady_tick(std::time::Duration::from_millis(80));

                let start_time = Instant::now();
                let mut stream = agent_engine.execute(&agent_context, &text);
                let mut full_response = String::new();
                let mut input_tokens: u32 = 0;
                let mut output_tokens: u32 = 0;
                let mut stop_reason = "end_turn".to_string();
                let mut first_token_received = false;
                let mut had_error = false;

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
                            break;
                        }
                    }
                }

                if !first_token_received && !had_error { spinner.finish_and_clear(); }
                if had_error { agent_context.conversation_history.pop(); continue; }

                let response_ms = start_time.elapsed().as_millis() as u64;
                println!();
                renderer.print_stats_footer(output_tokens, response_ms, &model);
                println!();

                // Persist assistant message
                agent_context.add_assistant_message(full_response.clone());
                let _ = state.chat_service.save_assistant_message(session_id, full_response.clone(), model.clone(), input_tokens, output_tokens, stop_reason, response_ms).await;

                session_manager.add_token_usage(input_tokens, output_tokens);
                let _ = state.chat_service.update_session_tokens(&session_id, input_tokens, output_tokens).await;
                session_manager.increment_turn();

                // Title generation after first exchange
                if first_assistant_response.is_none() {
                    first_assistant_response = Some(full_response.clone());
                    if let (Some(user_msg), Some(bot_msg)) = (&first_user_message, &first_assistant_response) {
                        if let Ok(title_provider) = create_provider(state, &model).await {
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
                    if let Ok(extract_provider) = create_provider(state, &model).await {
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
        if let Ok(extract_provider) = create_provider(state, &model).await {
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
