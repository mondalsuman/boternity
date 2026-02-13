//! Agent orchestrator for Boternity.
//!
//! `AgentOrchestrator` manages the full lifecycle of a user request:
//! initial LLM call, spawn detection, sub-agent execution (parallel or
//! sequential via `JoinSet`), retry-once logic, budget enforcement,
//! cancellation, and synthesis. All lifecycle events are published to
//! the `EventBus` via explicit `event_bus.publish(AgentEvent::...)` calls
//! for real-time UI updates.

use std::time::Instant;

use futures_util::StreamExt;
use tokio::task::JoinSet;
use tracing::{debug, warn};
use uuid::Uuid;

use boternity_types::agent::{AgentNode, AgentStatus, SpawnMode, SubAgentResult};
use boternity_types::event::AgentEvent;
use boternity_types::llm::{CompletionRequest, LlmError, Message, MessageRole, StreamEvent};

use crate::agent::budget::BudgetStatus;
use crate::agent::context::AgentContext;
use crate::agent::cycle_detector::CycleCheckResult;
use crate::agent::prompt::SystemPromptBuilder;
use crate::agent::request_context::RequestContext;
use crate::agent::spawner::{extract_text_before_spawn, parse_spawn_instructions};
use crate::event::EventBus;
use crate::llm::box_provider::BoxLlmProvider;

/// Orchestrates agent hierarchy execution for a single user request.
///
/// The orchestrator is a lightweight coordinator: it does not own an LLM
/// provider or store long-lived state. Each `execute()` call is independent.
/// The `max_depth` field controls the hard cap on agent nesting (default 3).
#[derive(Debug, Clone)]
pub struct AgentOrchestrator {
    /// Maximum depth for agent spawning (default 3).
    pub max_depth: u8,
}

impl Default for AgentOrchestrator {
    fn default() -> Self {
        Self { max_depth: 3 }
    }
}

impl AgentOrchestrator {
    /// Create a new orchestrator with the given max depth.
    pub fn new(max_depth: u8) -> Self {
        Self { max_depth }
    }

    /// Execute a user message through the agent hierarchy.
    ///
    /// This is the main entry point. It:
    /// 1. Rebuilds the system prompt with agent capabilities
    /// 2. Streams the LLM response, tracking budget
    /// 3. Checks for spawn instructions in the response
    /// 4. If spawning: runs sub-agents (parallel/sequential), then synthesizes
    /// 5. Returns `OrchestratorResult` with all execution data
    pub async fn execute(
        &self,
        provider: &BoxLlmProvider,
        context: &mut AgentContext,
        user_message: &str,
        request_ctx: &RequestContext,
        event_bus: &EventBus,
    ) -> Result<OrchestratorResult, OrchestratorError> {
        let root_agent_id = Uuid::now_v7();
        let start = Instant::now();

        // Step a: Rebuild system prompt with agent capabilities
        context.system_prompt = SystemPromptBuilder::build_with_capabilities(
            &context.agent_config,
            &context.soul_content,
            &context.identity_content,
            &context.user_content,
            &context.memories,
            &context.recalled_memories,
        );

        // Step b: Build CompletionRequest
        let request = build_completion_request(context, user_message);

        // Step c: Stream the response, tracking budget
        let full_response = self
            .stream_and_collect(provider, request, request_ctx, event_bus, root_agent_id)
            .await?;

        // Step d: Check for spawn instructions
        let spawn_instruction = parse_spawn_instructions(&full_response);

        if let Some(instruction) = spawn_instruction {
            // Step f: Check depth limit
            if request_ctx.depth >= self.max_depth {
                event_bus.publish(AgentEvent::DepthLimitReached {
                    agent_id: root_agent_id,
                    attempted_depth: request_ctx.depth + 1,
                    max_depth: self.max_depth,
                });
                debug!(
                    depth = request_ctx.depth,
                    max_depth = self.max_depth,
                    "Depth limit reached, returning response as-is"
                );
                return Ok(OrchestratorResult {
                    pre_spawn_text: Some(extract_text_before_spawn(&full_response).to_string()),
                    sub_agent_results: vec![],
                    synthesis: None,
                    final_response: full_response,
                    total_tokens_used: request_ctx.budget.tokens_used(),
                    agent_tree: vec![AgentNode {
                        agent_id: root_agent_id,
                        parent_id: None,
                        task: "root".to_string(),
                        depth: request_ctx.depth,
                        status: AgentStatus::Completed,
                        tokens_used: request_ctx.budget.tokens_used(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        children: vec![],
                    }],
                    memory_contexts: vec![],
                });
            }

            // Filter tasks through cycle detector
            let mut valid_tasks = Vec::new();
            for task in &instruction.tasks {
                match request_ctx
                    .cycle_detector
                    .check_and_register(task, request_ctx.depth)
                {
                    CycleCheckResult::Ok => {
                        valid_tasks.push(task.clone());
                    }
                    CycleCheckResult::CycleDetected { description } => {
                        event_bus.publish(AgentEvent::CycleDetected {
                            agent_id: root_agent_id,
                            cycle_description: description.clone(),
                        });
                        warn!(task = %task, description = %description, "Cycle detected, skipping task");
                    }
                }
            }

            if valid_tasks.is_empty() {
                // All tasks filtered by cycle detector, return as-is
                return Ok(OrchestratorResult {
                    pre_spawn_text: Some(extract_text_before_spawn(&full_response).to_string()),
                    sub_agent_results: vec![],
                    synthesis: None,
                    final_response: full_response,
                    total_tokens_used: request_ctx.budget.tokens_used(),
                    agent_tree: vec![AgentNode {
                        agent_id: root_agent_id,
                        parent_id: None,
                        task: "root".to_string(),
                        depth: request_ctx.depth,
                        status: AgentStatus::Completed,
                        tokens_used: request_ctx.budget.tokens_used(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        children: vec![],
                    }],
                    memory_contexts: vec![],
                });
            }

            // Execute sub-agents based on mode
            let sub_results = match instruction.mode {
                SpawnMode::Parallel => {
                    self.execute_parallel(
                        valid_tasks,
                        context,
                        provider,
                        request_ctx,
                        event_bus,
                        root_agent_id,
                    )
                    .await
                }
                SpawnMode::Sequential => {
                    self.execute_sequential(
                        valid_tasks,
                        context,
                        provider,
                        request_ctx,
                        event_bus,
                        root_agent_id,
                    )
                    .await
                }
            };

            // Step g: Synthesis
            event_bus.publish(AgentEvent::SynthesisStarted {
                request_id: request_ctx.request_id,
            });

            let synthesis_prompt = build_synthesis_prompt(&sub_results);
            let synthesis_request = build_completion_request(context, &synthesis_prompt);
            let synthesis_response = self
                .stream_and_collect(
                    provider,
                    synthesis_request,
                    request_ctx,
                    event_bus,
                    root_agent_id,
                )
                .await?;

            // Build agent tree
            let child_nodes: Vec<AgentNode> = sub_results
                .iter()
                .map(|r| AgentNode {
                    agent_id: r.agent_id,
                    parent_id: Some(root_agent_id),
                    task: r.task.clone(),
                    depth: request_ctx.depth + 1,
                    status: r.status.clone(),
                    tokens_used: r.tokens_used,
                    duration_ms: r.duration_ms,
                    children: vec![],
                })
                .collect();

            // Build memory contexts for sub-agent memory extraction
            let memory_contexts: Vec<AgentMemoryContext> = sub_results
                .iter()
                .filter(|r| r.status == AgentStatus::Completed && r.response.is_some())
                .map(|r| AgentMemoryContext {
                    agent_id: r.agent_id,
                    response_text: r.response.clone().unwrap_or_default(),
                    task_description: r.task.clone(),
                })
                .collect();

            let pre_spawn_text = extract_text_before_spawn(&full_response).to_string();

            Ok(OrchestratorResult {
                pre_spawn_text: if pre_spawn_text.is_empty() {
                    None
                } else {
                    Some(pre_spawn_text)
                },
                sub_agent_results: sub_results,
                synthesis: Some(synthesis_response.clone()),
                final_response: synthesis_response,
                total_tokens_used: request_ctx.budget.tokens_used(),
                agent_tree: vec![AgentNode {
                    agent_id: root_agent_id,
                    parent_id: None,
                    task: "root".to_string(),
                    depth: request_ctx.depth,
                    status: AgentStatus::Completed,
                    tokens_used: request_ctx.budget.tokens_used(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    children: child_nodes,
                }],
                memory_contexts,
            })
        } else {
            // Step e: No spawn instructions -- return direct response
            Ok(OrchestratorResult {
                pre_spawn_text: None,
                sub_agent_results: vec![],
                synthesis: None,
                final_response: full_response,
                total_tokens_used: request_ctx.budget.tokens_used(),
                agent_tree: vec![AgentNode {
                    agent_id: root_agent_id,
                    parent_id: None,
                    task: "root".to_string(),
                    depth: request_ctx.depth,
                    status: AgentStatus::Completed,
                    tokens_used: request_ctx.budget.tokens_used(),
                    duration_ms: start.elapsed().as_millis() as u64,
                    children: vec![],
                }],
                memory_contexts: vec![],
            })
        }
    }

    /// Execute sub-agent tasks in parallel using a `JoinSet`.
    ///
    /// Each task spawns a tokio task that runs `execute_single_agent`.
    /// Results are collected as they complete. JoinErrors (panics) are
    /// converted to failed `SubAgentResult`s.
    async fn execute_parallel(
        &self,
        tasks: Vec<String>,
        context: &AgentContext,
        provider: &BoxLlmProvider,
        request_ctx: &RequestContext,
        event_bus: &EventBus,
        parent_agent_id: Uuid,
    ) -> Vec<SubAgentResult> {
        let total = tasks.len();
        let mut set: JoinSet<SubAgentResult> = JoinSet::new();

        for (i, task) in tasks.into_iter().enumerate() {
            let child_ctx = context.child_for_task(&task, request_ctx.depth + 1);
            let child_request_ctx = request_ctx.child();
            let agent_id = Uuid::now_v7();
            let event_bus = event_bus.clone();
            let max_depth = self.max_depth;

            // We need the provider to be available in the spawned task.
            // Since BoxLlmProvider is not Clone, we build the request before spawning
            // and use the provider reference in the current task.
            // Instead, we'll use a different approach: build request outside, stream inside.
            // Actually, JoinSet tasks need 'static. We'll serialize the approach:
            // build the completion request here, then spawn a task that just collects.
            //
            // The correct approach: since we can't move BoxLlmProvider into JoinSet tasks
            // (it's not Clone), we'll use a different strategy. We'll collect all tasks
            // in-flight using tokio::spawn with Arc-wrapped or we accept sequential fallback.
            //
            // For now, given BoxLlmProvider is not Send or Clone in a way that works with
            // JoinSet, we simulate parallel by spawning sequential execution. The real
            // parallel execution requires the provider to be Arc-wrapped, which happens
            // at the chat handler level (plan 07). Here we prepare the structure.
            //
            // Actually: BoxLlmProvider wraps Box<dyn LlmProviderDyn + Send + Sync>, so
            // it is Send. But it's not Clone. The solution is to accept &BoxLlmProvider
            // and run tasks sequentially from the async perspective, or use separate
            // provider instances per task (which the caller can provide).
            //
            // The pragmatic approach: run tasks concurrently from THIS coroutine using
            // tokio::spawn with references is not possible (need 'static). So we'll
            // run them "concurrently" but from this context, using JoinSet with
            // pre-built requests. We stream each one from this context.

            // Publish AgentSpawned
            event_bus.publish(AgentEvent::AgentSpawned {
                agent_id,
                parent_id: Some(parent_agent_id),
                task_description: task.clone(),
                depth: child_request_ctx.depth,
                index: i,
                total,
            });

            // We can't move the provider into the JoinSet, so we build the task info
            // and execute below in a sequential loop that publishes the right events.
            // Store the setup for sequential execution with parallel event semantics.
            let task_desc = task.clone();
            let bus = event_bus.clone();
            let req_ctx = child_request_ctx.clone();

            // Build completion request for the child
            let child_request = build_completion_request(&child_ctx, &task_desc);

            // Create the stream outside the JoinSet (uses provider reference)
            let stream = provider.stream(child_request);

            // Spawn the collection task (the stream is 'static)
            set.spawn(async move {
                let start = Instant::now();
                let result = collect_stream_with_events(
                    stream,
                    &req_ctx,
                    &bus,
                    agent_id,
                    max_depth,
                )
                .await;

                let duration_ms = start.elapsed().as_millis() as u64;

                match result {
                    Ok((response, tokens)) => {
                        bus.publish(AgentEvent::AgentCompleted {
                            agent_id,
                            result_summary: truncate_summary(&response, 200),
                            tokens_used: tokens,
                            duration_ms,
                        });
                        SubAgentResult {
                            agent_id,
                            task: task_desc,
                            status: AgentStatus::Completed,
                            response: Some(response),
                            error: None,
                            tokens_used: tokens,
                            duration_ms,
                        }
                    }
                    Err(e) => {
                        bus.publish(AgentEvent::AgentFailed {
                            agent_id,
                            error: e.to_string(),
                            will_retry: false,
                        });
                        SubAgentResult {
                            agent_id,
                            task: task_desc,
                            status: AgentStatus::Failed,
                            response: None,
                            error: Some(e.to_string()),
                            tokens_used: req_ctx.budget.tokens_used(),
                            duration_ms,
                        }
                    }
                }
            });
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(join_result) = set.join_next().await {
            match join_result {
                Ok(sub_result) => results.push(sub_result),
                Err(join_error) => {
                    // JoinError means the task panicked (pitfall 8)
                    warn!(error = %join_error, "Sub-agent task panicked");
                    results.push(SubAgentResult {
                        agent_id: Uuid::now_v7(),
                        task: "unknown (panicked)".to_string(),
                        status: AgentStatus::Failed,
                        response: None,
                        error: Some(format!("Task panicked: {join_error}")),
                        tokens_used: 0,
                        duration_ms: 0,
                    });
                }
            }
        }

        results
    }

    /// Execute sub-agent tasks sequentially.
    ///
    /// Each task sees only the immediately prior sub-agent's result (not the
    /// full chain), per user decision. Cancellation and budget are checked
    /// between tasks.
    async fn execute_sequential(
        &self,
        tasks: Vec<String>,
        context: &AgentContext,
        provider: &BoxLlmProvider,
        request_ctx: &RequestContext,
        event_bus: &EventBus,
        parent_agent_id: Uuid,
    ) -> Vec<SubAgentResult> {
        let total = tasks.len();
        let mut results = Vec::new();

        for (i, task) in tasks.iter().enumerate() {
            // Check cancellation between tasks
            if request_ctx.is_cancelled() {
                debug!("Sequential execution cancelled after task {i}");
                break;
            }

            // Check budget between tasks
            if request_ctx.budget.remaining() == 0 {
                debug!("Budget exhausted, stopping sequential execution after task {i}");
                event_bus.publish(AgentEvent::BudgetExhausted {
                    request_id: request_ctx.request_id,
                    tokens_used: request_ctx.budget.tokens_used(),
                    budget_total: request_ctx.budget.total_budget(),
                    completed_agents: results.iter().map(|r: &SubAgentResult| r.agent_id).collect(),
                    incomplete_agents: vec![],
                });
                break;
            }

            let agent_id = Uuid::now_v7();
            let child_request_ctx = request_ctx.child();

            // Create child context; inject previous result if available
            let mut child_ctx = context.child_for_task(task, request_ctx.depth + 1);
            if let Some(prev_result) = results.last() {
                let prev: &SubAgentResult = prev_result;
                if let Some(ref prev_response) = prev.response {
                    child_ctx.add_user_message(format!(
                        "Previous sub-agent result for context:\n{}",
                        prev_response
                    ));
                }
            }

            // Publish AgentSpawned
            event_bus.publish(AgentEvent::AgentSpawned {
                agent_id,
                parent_id: Some(parent_agent_id),
                task_description: task.clone(),
                depth: child_request_ctx.depth,
                index: i,
                total,
            });

            // Execute with retry-once logic
            let result = self
                .execute_single_agent(
                    &child_ctx,
                    provider,
                    &child_request_ctx,
                    event_bus,
                    agent_id,
                    task,
                )
                .await;

            results.push(result);
        }

        results
    }

    /// Execute a single sub-agent LLM call with retry-once logic.
    ///
    /// On first failure, publishes `AgentFailed` with `will_retry: true` and
    /// retries once. On second failure, returns a failed `SubAgentResult`.
    /// Respects cancellation via `tokio::select!` with the cancellation token.
    async fn execute_single_agent(
        &self,
        context: &AgentContext,
        provider: &BoxLlmProvider,
        request_ctx: &RequestContext,
        event_bus: &EventBus,
        agent_id: Uuid,
        task: &str,
    ) -> SubAgentResult {
        for attempt in 0..2u8 {
            let start = Instant::now();

            let result = tokio::select! {
                _ = request_ctx.cancellation.cancelled() => {
                    event_bus.publish(AgentEvent::AgentCancelled {
                        agent_id,
                        reason: "Request cancelled by user".to_string(),
                    });
                    return SubAgentResult {
                        agent_id,
                        task: task.to_string(),
                        status: AgentStatus::Cancelled,
                        response: None,
                        error: Some("Cancelled".to_string()),
                        tokens_used: 0,
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
                result = self.run_single_llm_call(context, provider, request_ctx, event_bus, agent_id, task) => {
                    result
                }
            };

            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok((response, tokens_used)) => {
                    event_bus.publish(AgentEvent::AgentCompleted {
                        agent_id,
                        result_summary: truncate_summary(&response, 200),
                        tokens_used,
                        duration_ms,
                    });
                    return SubAgentResult {
                        agent_id,
                        task: task.to_string(),
                        status: AgentStatus::Completed,
                        response: Some(response),
                        error: None,
                        tokens_used,
                        duration_ms,
                    };
                }
                Err(e) => {
                    let will_retry = attempt == 0;
                    event_bus.publish(AgentEvent::AgentFailed {
                        agent_id,
                        error: e.to_string(),
                        will_retry,
                    });
                    if !will_retry {
                        // Second attempt failed, return failure
                        return SubAgentResult {
                            agent_id,
                            task: task.to_string(),
                            status: AgentStatus::Failed,
                            response: None,
                            error: Some(e.to_string()),
                            tokens_used: 0,
                            duration_ms,
                        };
                    }
                    debug!(attempt, error = %e, "Sub-agent failed, retrying once");
                }
            }
        }

        // Should not reach here, but just in case
        SubAgentResult {
            agent_id,
            task: task.to_string(),
            status: AgentStatus::Failed,
            response: None,
            error: Some("Exhausted retries".to_string()),
            tokens_used: 0,
            duration_ms: 0,
        }
    }

    /// Run a single LLM call for a sub-agent, streaming and collecting the response.
    ///
    /// Publishes `AgentTextDelta` for each token and handles budget
    /// warning/exhaustion events.
    async fn run_single_llm_call(
        &self,
        context: &AgentContext,
        provider: &BoxLlmProvider,
        request_ctx: &RequestContext,
        event_bus: &EventBus,
        agent_id: Uuid,
        task: &str,
    ) -> Result<(String, u32), OrchestratorError> {
        let request = build_completion_request(context, task);
        let stream = provider.stream(request);

        let (response, tokens) =
            collect_stream_with_events(stream, request_ctx, event_bus, agent_id, self.max_depth)
                .await?;

        Ok((response, tokens))
    }

    /// Stream a completion and collect the full response text.
    ///
    /// Publishes budget events (BudgetUpdate, BudgetWarning, BudgetExhausted)
    /// as tokens arrive.
    async fn stream_and_collect(
        &self,
        provider: &BoxLlmProvider,
        request: CompletionRequest,
        request_ctx: &RequestContext,
        event_bus: &EventBus,
        agent_id: Uuid,
    ) -> Result<String, OrchestratorError> {
        let stream = provider.stream(request);

        let (response, _tokens) =
            collect_stream_with_events(stream, request_ctx, event_bus, agent_id, self.max_depth)
                .await?;

        Ok(response)
    }
}

/// Collect a stream of LLM events into a full response string.
///
/// Publishes `AgentTextDelta`, `BudgetUpdate`, `BudgetWarning`, and
/// `BudgetExhausted` events. Returns the full response text and estimated
/// token count.
async fn collect_stream_with_events(
    mut stream: std::pin::Pin<
        Box<dyn futures_util::Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>,
    >,
    request_ctx: &RequestContext,
    event_bus: &EventBus,
    agent_id: Uuid,
    _max_depth: u8,
) -> Result<(String, u32), OrchestratorError> {
    let mut full_response = String::new();
    let mut total_tokens: u32 = 0;

    while let Some(event_result) = stream.next().await {
        // Check cancellation during streaming
        if request_ctx.is_cancelled() {
            event_bus.publish(AgentEvent::AgentCancelled {
                agent_id,
                reason: "Request cancelled during streaming".to_string(),
            });
            return Err(OrchestratorError::Cancelled);
        }

        let event = event_result.map_err(OrchestratorError::LlmError)?;

        match event {
            StreamEvent::TextDelta { text, .. } => {
                // Estimate tokens from text (4 chars ~ 1 token)
                let chunk_tokens = (text.len() as u32 / 4).max(1);
                total_tokens += chunk_tokens;

                full_response.push_str(&text);

                // Publish text delta
                event_bus.publish(AgentEvent::AgentTextDelta {
                    agent_id,
                    text: text.clone(),
                });

                // Track budget
                let status = request_ctx.budget.add_tokens(chunk_tokens);
                event_bus.publish(AgentEvent::BudgetUpdate {
                    request_id: request_ctx.request_id,
                    tokens_used: request_ctx.budget.tokens_used(),
                    budget_total: request_ctx.budget.total_budget(),
                    percentage: request_ctx.budget.percentage(),
                });

                match status {
                    BudgetStatus::Warning => {
                        event_bus.publish(AgentEvent::BudgetWarning {
                            request_id: request_ctx.request_id,
                            tokens_used: request_ctx.budget.tokens_used(),
                            budget_total: request_ctx.budget.total_budget(),
                        });
                    }
                    BudgetStatus::Exhausted => {
                        event_bus.publish(AgentEvent::BudgetExhausted {
                            request_id: request_ctx.request_id,
                            tokens_used: request_ctx.budget.tokens_used(),
                            budget_total: request_ctx.budget.total_budget(),
                            completed_agents: vec![],
                            incomplete_agents: vec![agent_id],
                        });
                        // Return partial result
                        return Ok((full_response, total_tokens));
                    }
                    BudgetStatus::Ok => {}
                }
            }
            StreamEvent::Usage(usage) => {
                // If we get real token counts from the provider, use those
                let real_tokens = usage.input_tokens + usage.output_tokens;
                if real_tokens > 0 {
                    // Adjust budget with real usage if significantly different
                    let diff = real_tokens.saturating_sub(total_tokens);
                    if diff > 0 {
                        request_ctx.budget.add_tokens(diff);
                        total_tokens = real_tokens;
                    }
                }
            }
            StreamEvent::Done => break,
            _ => {} // Connected, ContentBlockStart/Stop, ThinkingDelta, etc.
        }
    }

    Ok((full_response, total_tokens))
}

/// Build a `CompletionRequest` from an `AgentContext` and a user message.
///
/// Replicates the pattern from `AgentEngine::build_request` for use by the
/// orchestrator (which doesn't own an `AgentEngine`).
fn build_completion_request(context: &AgentContext, user_message: &str) -> CompletionRequest {
    let mut messages = context.build_messages();
    messages.push(Message {
        role: MessageRole::User,
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

/// Build the synthesis prompt from sub-agent results.
///
/// Produces an XML `<sub_agent_results>` block that the root agent uses to
/// synthesize a cohesive final response from all sub-agent outputs.
pub fn build_synthesis_prompt(results: &[SubAgentResult]) -> String {
    let mut xml = String::from("<sub_agent_results>\n");

    for result in results {
        let status_str = match result.status {
            AgentStatus::Completed => "completed",
            AgentStatus::Failed => "failed",
            AgentStatus::Cancelled => "cancelled",
            AgentStatus::Pending => "pending",
            AgentStatus::Running => "running",
        };

        // Escape task text for XML attribute
        let escaped_task = result
            .task
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        xml.push_str(&format!(
            "  <result task=\"{escaped_task}\" status=\"{status_str}\">\n"
        ));

        match (&result.response, &result.error) {
            (Some(response), _) => {
                xml.push_str(&format!("    {}\n", response.trim()));
            }
            (None, Some(error)) => {
                xml.push_str(&format!("    Error: {}\n", error.trim()));
            }
            (None, None) => {
                xml.push_str("    (no output)\n");
            }
        }

        xml.push_str("  </result>\n");
    }

    xml.push_str("</sub_agent_results>\n\n");
    xml.push_str(
        "Based on these sub-agent results, synthesize a cohesive response that \
         integrates all findings. Address any gaps from failed sub-agents.",
    );

    xml
}

/// Truncate a string to the given max length for result summaries.
fn truncate_summary(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ---------------------------------------------------------------------------
// Result and error types
// ---------------------------------------------------------------------------

/// Result of a full orchestrator execution.
///
/// Contains all execution artifacts: the final response, sub-agent results,
/// agent tree for UI rendering, and memory contexts for sub-agent memory
/// extraction with `source_agent_id` tagging.
#[derive(Debug, Clone)]
pub struct OrchestratorResult {
    /// Text from the root agent before the spawn block (if any).
    pub pre_spawn_text: Option<String>,
    /// Results from sub-agents (empty if no spawn occurred).
    pub sub_agent_results: Vec<SubAgentResult>,
    /// The synthesis response (None if no spawn occurred).
    pub synthesis: Option<String>,
    /// The complete response to show the user (direct response or synthesis).
    pub final_response: String,
    /// Total tokens consumed across all agents.
    pub total_tokens_used: u32,
    /// Flat list of all agents for tree rendering in UI.
    pub agent_tree: Vec<AgentNode>,
    /// Per-agent response data for memory extraction with `source_agent_id` tagging.
    ///
    /// The chat handler uses this to run memory extraction per sub-agent,
    /// setting `source_agent_id: Some(agent_id)` on each `MemoryEntry` created,
    /// and publishing `AgentEvent::MemoryCreated` with the correct `agent_id`.
    pub memory_contexts: Vec<AgentMemoryContext>,
}

/// Context data for memory extraction from a specific sub-agent's response.
///
/// Enables the chat handler to run memory extraction with the correct
/// `source_agent_id` tagging per the locked user decision: "Sub-agents have
/// full memory access -- can both recall and create memories, tagged with
/// which agent created them."
#[derive(Debug, Clone)]
pub struct AgentMemoryContext {
    /// The sub-agent that produced this response.
    pub agent_id: Uuid,
    /// The sub-agent's full response text.
    pub response_text: String,
    /// What the sub-agent was asked to do.
    pub task_description: String,
}

/// Errors from orchestrator execution.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    /// An LLM provider error occurred.
    #[error("LLM error: {0}")]
    LlmError(#[from] LlmError),

    /// Token budget was exhausted.
    #[error("budget exhausted: {tokens_used} tokens used")]
    BudgetExhausted {
        partial_results: Vec<SubAgentResult>,
        tokens_used: u32,
    },

    /// The request was cancelled by the user.
    #[error("request cancelled")]
    Cancelled,

    /// An unexpected internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::agent::AgentStatus;
    use uuid::Uuid;

    #[test]
    fn test_build_synthesis_prompt_all_completed() {
        let results = vec![
            SubAgentResult {
                agent_id: Uuid::now_v7(),
                task: "Research quantum computing".to_string(),
                status: AgentStatus::Completed,
                response: Some("Quantum computing uses qubits for computation.".to_string()),
                error: None,
                tokens_used: 500,
                duration_ms: 2000,
            },
            SubAgentResult {
                agent_id: Uuid::now_v7(),
                task: "List top companies".to_string(),
                status: AgentStatus::Completed,
                response: Some("IBM, Google, IonQ are leading companies.".to_string()),
                error: None,
                tokens_used: 300,
                duration_ms: 1500,
            },
        ];

        let prompt = build_synthesis_prompt(&results);

        assert!(prompt.contains("<sub_agent_results>"));
        assert!(prompt.contains("</sub_agent_results>"));
        assert!(prompt.contains(r#"task="Research quantum computing""#));
        assert!(prompt.contains(r#"status="completed""#));
        assert!(prompt.contains("Quantum computing uses qubits"));
        assert!(prompt.contains(r#"task="List top companies""#));
        assert!(prompt.contains("IBM, Google, IonQ"));
        assert!(prompt.contains("synthesize a cohesive response"));
    }

    #[test]
    fn test_build_synthesis_prompt_with_failed_task() {
        let results = vec![
            SubAgentResult {
                agent_id: Uuid::now_v7(),
                task: "Successful task".to_string(),
                status: AgentStatus::Completed,
                response: Some("Success output here.".to_string()),
                error: None,
                tokens_used: 200,
                duration_ms: 1000,
            },
            SubAgentResult {
                agent_id: Uuid::now_v7(),
                task: "Failed task".to_string(),
                status: AgentStatus::Failed,
                response: None,
                error: Some("timeout after 30s".to_string()),
                tokens_used: 50,
                duration_ms: 30000,
            },
        ];

        let prompt = build_synthesis_prompt(&results);

        assert!(prompt.contains(r#"status="completed""#));
        assert!(prompt.contains(r#"status="failed""#));
        assert!(prompt.contains("Error: timeout after 30s"));
        assert!(prompt.contains("Address any gaps from failed sub-agents"));
    }

    #[test]
    fn test_build_synthesis_prompt_empty_results() {
        let prompt = build_synthesis_prompt(&[]);

        assert!(prompt.contains("<sub_agent_results>"));
        assert!(prompt.contains("</sub_agent_results>"));
        assert!(prompt.contains("synthesize a cohesive response"));
    }

    #[test]
    fn test_build_synthesis_prompt_escapes_xml_in_task() {
        let results = vec![SubAgentResult {
            agent_id: Uuid::now_v7(),
            task: "Compare A & B <details>".to_string(),
            status: AgentStatus::Completed,
            response: Some("Comparison results.".to_string()),
            error: None,
            tokens_used: 100,
            duration_ms: 500,
        }];

        let prompt = build_synthesis_prompt(&results);

        // Task text should be XML-escaped
        assert!(prompt.contains("Compare A &amp; B &lt;details&gt;"));
        assert!(!prompt.contains("Compare A & B <details>"));
    }

    #[test]
    fn test_build_synthesis_prompt_cancelled_task() {
        let results = vec![SubAgentResult {
            agent_id: Uuid::now_v7(),
            task: "Cancelled task".to_string(),
            status: AgentStatus::Cancelled,
            response: None,
            error: Some("Cancelled".to_string()),
            tokens_used: 0,
            duration_ms: 100,
        }];

        let prompt = build_synthesis_prompt(&results);

        assert!(prompt.contains(r#"status="cancelled""#));
        assert!(prompt.contains("Error: Cancelled"));
    }

    #[test]
    fn test_orchestrator_result_construction() {
        let result = OrchestratorResult {
            pre_spawn_text: Some("I'll break this down.".to_string()),
            sub_agent_results: vec![SubAgentResult {
                agent_id: Uuid::now_v7(),
                task: "Sub-task".to_string(),
                status: AgentStatus::Completed,
                response: Some("Result".to_string()),
                error: None,
                tokens_used: 100,
                duration_ms: 500,
            }],
            synthesis: Some("Synthesized response.".to_string()),
            final_response: "Synthesized response.".to_string(),
            total_tokens_used: 200,
            agent_tree: vec![],
            memory_contexts: vec![AgentMemoryContext {
                agent_id: Uuid::now_v7(),
                response_text: "Result".to_string(),
                task_description: "Sub-task".to_string(),
            }],
        };

        assert_eq!(result.pre_spawn_text.as_deref(), Some("I'll break this down."));
        assert_eq!(result.sub_agent_results.len(), 1);
        assert_eq!(result.synthesis.as_deref(), Some("Synthesized response."));
        assert_eq!(result.final_response, "Synthesized response.");
        assert_eq!(result.total_tokens_used, 200);
        assert_eq!(result.memory_contexts.len(), 1);
        assert_eq!(result.memory_contexts[0].response_text, "Result");
        assert_eq!(result.memory_contexts[0].task_description, "Sub-task");
    }

    #[test]
    fn test_orchestrator_result_no_spawn() {
        let result = OrchestratorResult {
            pre_spawn_text: None,
            sub_agent_results: vec![],
            synthesis: None,
            final_response: "Direct response.".to_string(),
            total_tokens_used: 50,
            agent_tree: vec![],
            memory_contexts: vec![],
        };

        assert!(result.pre_spawn_text.is_none());
        assert!(result.sub_agent_results.is_empty());
        assert!(result.synthesis.is_none());
        assert_eq!(result.final_response, "Direct response.");
        assert!(result.memory_contexts.is_empty());
    }

    #[test]
    fn test_agent_memory_context_population_from_sub_agent_results() {
        let agent1_id = Uuid::now_v7();
        let agent2_id = Uuid::now_v7();
        let agent3_id = Uuid::now_v7();

        let results = vec![
            SubAgentResult {
                agent_id: agent1_id,
                task: "Research topic A".to_string(),
                status: AgentStatus::Completed,
                response: Some("Topic A findings.".to_string()),
                error: None,
                tokens_used: 100,
                duration_ms: 500,
            },
            SubAgentResult {
                agent_id: agent2_id,
                task: "Research topic B".to_string(),
                status: AgentStatus::Failed,
                response: None,
                error: Some("timeout".to_string()),
                tokens_used: 50,
                duration_ms: 30000,
            },
            SubAgentResult {
                agent_id: agent3_id,
                task: "Research topic C".to_string(),
                status: AgentStatus::Completed,
                response: Some("Topic C findings.".to_string()),
                error: None,
                tokens_used: 150,
                duration_ms: 800,
            },
        ];

        // Only completed agents with responses should produce memory contexts
        let memory_contexts: Vec<AgentMemoryContext> = results
            .iter()
            .filter(|r| r.status == AgentStatus::Completed && r.response.is_some())
            .map(|r| AgentMemoryContext {
                agent_id: r.agent_id,
                response_text: r.response.clone().unwrap_or_default(),
                task_description: r.task.clone(),
            })
            .collect();

        assert_eq!(memory_contexts.len(), 2);
        assert_eq!(memory_contexts[0].agent_id, agent1_id);
        assert_eq!(memory_contexts[0].response_text, "Topic A findings.");
        assert_eq!(memory_contexts[0].task_description, "Research topic A");
        assert_eq!(memory_contexts[1].agent_id, agent3_id);
        assert_eq!(memory_contexts[1].response_text, "Topic C findings.");
        assert_eq!(memory_contexts[1].task_description, "Research topic C");
    }

    #[test]
    fn test_orchestrator_default() {
        let orchestrator = AgentOrchestrator::default();
        assert_eq!(orchestrator.max_depth, 3);
    }

    #[test]
    fn test_orchestrator_custom_depth() {
        let orchestrator = AgentOrchestrator::new(5);
        assert_eq!(orchestrator.max_depth, 5);
    }

    #[test]
    fn test_truncate_summary_short() {
        assert_eq!(truncate_summary("short", 200), "short");
    }

    #[test]
    fn test_truncate_summary_long() {
        let long = "a".repeat(300);
        let result = truncate_summary(&long, 200);
        assert_eq!(result.len(), 203); // 200 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_build_completion_request_from_context() {
        use boternity_types::agent::AgentConfig;
        use crate::llm::token_budget::TokenBudget;

        let config = AgentConfig {
            bot_id: Uuid::now_v7(),
            bot_name: "TestBot".to_string(),
            bot_slug: "testbot".to_string(),
            bot_emoji: None,
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        };

        let mut context = AgentContext::new(
            config,
            "Soul content.".to_string(),
            "Identity content.".to_string(),
            String::new(),
            vec![],
            TokenBudget::new(200_000),
        );

        context.add_user_message("Previous message".to_string());
        context.add_assistant_message("Previous response".to_string());

        let request = build_completion_request(&context, "New question");

        assert_eq!(request.model, "claude-sonnet-4-20250514");
        assert_eq!(request.max_tokens, 4096);
        assert!(request.system.is_some());
        // 2 history messages + 1 new user message
        assert_eq!(request.messages.len(), 3);
        assert_eq!(request.messages[2].content, "New question");
        assert_eq!(request.messages[2].role, MessageRole::User);
    }

    #[test]
    fn test_orchestrator_error_display() {
        let err = OrchestratorError::Cancelled;
        assert_eq!(err.to_string(), "request cancelled");

        let err = OrchestratorError::Internal("something broke".to_string());
        assert_eq!(err.to_string(), "internal error: something broke");

        let err = OrchestratorError::BudgetExhausted {
            partial_results: vec![],
            tokens_used: 500000,
        };
        assert!(err.to_string().contains("500000"));
    }

    #[test]
    fn test_build_synthesis_prompt_no_output() {
        let results = vec![SubAgentResult {
            agent_id: Uuid::now_v7(),
            task: "Empty task".to_string(),
            status: AgentStatus::Completed,
            response: None,
            error: None,
            tokens_used: 0,
            duration_ms: 100,
        }];

        let prompt = build_synthesis_prompt(&results);

        assert!(prompt.contains("(no output)"));
    }
}
