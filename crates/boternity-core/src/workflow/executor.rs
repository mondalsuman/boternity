//! Workflow executor: wave-based parallel DAG execution with durable checkpointing.
//!
//! The `DagExecutor` processes workflow steps in topological wave order. Steps
//! within the same wave run concurrently via `tokio::JoinSet`. Each step is
//! checkpointed to SQLite before and after execution, enabling crash recovery
//! by resuming from the last completed step.
//!
//! # Execution flow
//!
//! 1. Create a `WorkflowRun` record (or load existing for resume).
//! 2. Build an execution plan via `build_execution_plan` (waves of steps).
//! 3. For each wave, spawn all steps as parallel tasks.
//! 4. Each step: checkpoint start -> evaluate condition -> run step -> checkpoint result.
//! 5. Accumulate outputs in `WorkflowContext`.
//! 6. On completion/failure/cancellation, update the run record.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use boternity_types::event::AgentEvent;
use boternity_types::workflow::{
    WorkflowDefinition, WorkflowRun, WorkflowRunStatus, StepDefinition,
};
use chrono::Utc;
use dashmap::DashMap;
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::event::bus::EventBus;
use crate::repository::workflow::WorkflowRepository;

use super::checkpoint::{CheckpointError, CheckpointManager};
use super::context::WorkflowContext;
use super::dag::build_execution_plan;
use super::definition::WorkflowError;
use super::expression::WorkflowEvaluator;
use super::step_runner::StepRunner;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default workflow-level timeout (30 minutes).
pub const DEFAULT_WORKFLOW_TIMEOUT_SECS: u64 = 1800;

/// Default step-level timeout (5 minutes).
pub const DEFAULT_STEP_TIMEOUT_SECS: u64 = 300;

// ---------------------------------------------------------------------------
// WorkflowExecutor trait
// ---------------------------------------------------------------------------

/// Trait for workflow execution engines.
///
/// Uses RPITIT (return-position `impl Trait` in traits) for async methods,
/// consistent with the project's Rust 2024 edition approach.
pub trait WorkflowExecutor: Send + Sync {
    /// Execute a workflow definition from the beginning.
    fn execute(
        &self,
        definition: &WorkflowDefinition,
        trigger_type: &str,
        trigger_payload: Option<Value>,
    ) -> impl std::future::Future<Output = Result<ExecutionResult, ExecutorError>> + Send;

    /// Resume a crashed or paused workflow run from its last checkpoint.
    fn resume(
        &self,
        run_id: Uuid,
        definition: &WorkflowDefinition,
    ) -> impl std::future::Future<Output = Result<ExecutionResult, ExecutorError>> + Send;

    /// Cancel a running workflow.
    fn cancel(
        &self,
        run_id: Uuid,
    ) -> impl std::future::Future<Output = Result<(), ExecutorError>> + Send;
}

// ---------------------------------------------------------------------------
// ExecutionResult
// ---------------------------------------------------------------------------

/// Result of a completed (or paused) workflow execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The workflow run ID.
    pub run_id: Uuid,
    /// Final status of the run.
    pub status: WorkflowRunStatus,
    /// Accumulated context (step outputs, variables).
    pub context: WorkflowContext,
    /// IDs of steps that completed.
    pub completed_steps: Vec<String>,
    /// Error message if the workflow failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// DagExecutor
// ---------------------------------------------------------------------------

/// Wave-based DAG executor with durable checkpointing.
///
/// Generic over `R: WorkflowRepository` for storage flexibility.
pub struct DagExecutor<R: WorkflowRepository> {
    checkpoint: Arc<CheckpointManager<R>>,
    event_bus: EventBus,
    evaluator: WorkflowEvaluator,
    step_runner: Arc<StepRunner>,
    /// Per-workflow concurrency semaphores keyed by workflow name.
    concurrency_semaphores: DashMap<String, Arc<Semaphore>>,
    /// Cancellation tokens keyed by run_id.
    cancellation_tokens: DashMap<Uuid, tokio_util::sync::CancellationToken>,
}

impl<R: WorkflowRepository + 'static> DagExecutor<R> {
    /// Create a new DAG executor with placeholder execution context.
    pub fn new(
        repo: R,
        event_bus: EventBus,
        data_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            checkpoint: Arc::new(CheckpointManager::new(repo)),
            event_bus,
            evaluator: WorkflowEvaluator::new(),
            step_runner: Arc::new(StepRunner::new(data_dir)),
            concurrency_semaphores: DashMap::new(),
            cancellation_tokens: DashMap::new(),
        }
    }

    /// Create a new DAG executor with a real execution context for live service wiring.
    ///
    /// The execution context provides concrete implementations for agent chat,
    /// skill invocation, and HTTP request execution instead of placeholders.
    pub fn with_execution_context(
        repo: R,
        event_bus: EventBus,
        data_dir: std::path::PathBuf,
        exec_ctx: Arc<dyn super::step_runner::StepExecutionContext>,
    ) -> Self {
        Self {
            checkpoint: Arc::new(CheckpointManager::new(repo)),
            event_bus,
            evaluator: WorkflowEvaluator::new(),
            step_runner: Arc::new(StepRunner::with_context(data_dir, exec_ctx)),
            concurrency_semaphores: DashMap::new(),
            cancellation_tokens: DashMap::new(),
        }
    }

    /// Acquire a concurrency permit for the workflow (if concurrency is limited).
    async fn acquire_concurrency_permit(
        &self,
        definition: &WorkflowDefinition,
    ) -> Result<Option<tokio::sync::OwnedSemaphorePermit>, ExecutorError> {
        if let Some(max) = definition.concurrency {
            let semaphore = self
                .concurrency_semaphores
                .entry(definition.name.clone())
                .or_insert_with(|| Arc::new(Semaphore::new(max as usize)))
                .clone();

            let permit = semaphore.try_acquire_owned().map_err(|_| {
                ExecutorError::Workflow(WorkflowError::ConcurrencyLimitReached)
            })?;

            Ok(Some(permit))
        } else {
            Ok(None)
        }
    }

    /// Execute waves of steps, skipping already-completed steps.
    async fn execute_waves(
        &self,
        definition: &WorkflowDefinition,
        run_id: Uuid,
        ctx: &mut WorkflowContext,
        completed_steps: &HashSet<String>,
        cancel_token: &tokio_util::sync::CancellationToken,
    ) -> Result<WorkflowRunStatus, ExecutorError> {
        // Build execution plan and immediately clone steps into owned vectors
        // so that spawned tasks don't borrow from `definition`.
        let waves_refs = build_execution_plan(&definition.steps)
            .map_err(ExecutorError::Workflow)?;
        let waves: Vec<Vec<StepDefinition>> = waves_refs
            .into_iter()
            .map(|wave| wave.into_iter().cloned().collect())
            .collect();

        let workflow_timeout = Duration::from_secs(
            definition.timeout_secs.unwrap_or(DEFAULT_WORKFLOW_TIMEOUT_SECS),
        );

        let execution = async {
            for (wave_idx, wave) in waves.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    return Ok(WorkflowRunStatus::Cancelled);
                }

                tracing::debug!(
                    run_id = %run_id,
                    wave = wave_idx,
                    steps = wave.len(),
                    "processing wave"
                );

                let mut join_set = JoinSet::new();

                for step_def in wave {
                    // Skip already-completed steps (crash recovery)
                    if completed_steps.contains(&step_def.id) {
                        tracing::debug!(
                            step_id = step_def.id.as_str(),
                            "skipping completed step"
                        );
                        continue;
                    }

                    // Evaluate step condition
                    if let Some(condition) = &step_def.condition {
                        let should_run = self
                            .evaluator
                            .evaluate_in_workflow_context(condition, ctx)
                            .map_err(|e| {
                                ExecutorError::Workflow(WorkflowError::ExpressionError(
                                    e.to_string(),
                                ))
                            })?;

                        if !should_run {
                            self.checkpoint
                                .checkpoint_step_skipped(
                                    run_id,
                                    &step_def.id,
                                    &step_def.name,
                                )
                                .await
                                .map_err(ExecutorError::Checkpoint)?;
                            continue;
                        }
                    }

                    // Clone what we need for the spawned task
                    let step = step_def.clone();
                    let checkpoint = Arc::clone(&self.checkpoint);
                    let runner = Arc::clone(&self.step_runner);
                    let step_ctx = ctx.clone();
                    let step_timeout = Duration::from_secs(
                        step.timeout_secs.unwrap_or(DEFAULT_STEP_TIMEOUT_SECS),
                    );
                    let token = cancel_token.clone();
                    let event_bus = self.event_bus.clone();

                    join_set.spawn(async move {
                        if token.is_cancelled() {
                            return Err(ExecutorError::Cancelled);
                        }

                        // Publish step started event
                        let step_type_str = format!("{:?}", step.step_type).to_lowercase();
                        event_bus.publish(AgentEvent::WorkflowStepStarted {
                            run_id,
                            step_id: step.id.clone(),
                            step_name: step.name.clone(),
                            step_type: step_type_str,
                        });

                        // Checkpoint: step start
                        let log_id = checkpoint
                            .checkpoint_step_start(run_id, &step.id, &step.name, 1)
                            .await
                            .map_err(ExecutorError::Checkpoint)?;

                        let start_instant = std::time::Instant::now();

                        // Execute with timeout
                        let result = tokio::time::timeout(
                            step_timeout,
                            runner.run(&step, &step_ctx),
                        )
                        .await;

                        let elapsed_ms = start_instant.elapsed().as_millis() as u64;

                        match result {
                            Ok(Ok(output)) => {
                                // Checkpoint: step complete
                                let output_value = output.to_value();
                                checkpoint
                                    .checkpoint_step_complete(log_id, Some(&output_value))
                                    .await
                                    .map_err(ExecutorError::Checkpoint)?;

                                // Publish step completed event
                                event_bus.publish(AgentEvent::WorkflowStepCompleted {
                                    run_id,
                                    step_id: step.id.clone(),
                                    step_name: step.name.clone(),
                                    duration_ms: elapsed_ms,
                                });

                                Ok((step.id.clone(), output))
                            }
                            Ok(Err(step_err)) => {
                                // Check for approval gate
                                if step_err.is_approval_required() {
                                    checkpoint
                                        .checkpoint_step_waiting_approval(log_id)
                                        .await
                                        .map_err(ExecutorError::Checkpoint)?;
                                    return Err(ExecutorError::ApprovalRequired {
                                        step_id: step.id.clone(),
                                        prompt: step_err.approval_prompt().unwrap_or_default(),
                                    });
                                }

                                // Checkpoint: step failed
                                let err_msg = step_err.to_string();
                                checkpoint
                                    .checkpoint_step_failed(log_id, &err_msg)
                                    .await
                                    .map_err(ExecutorError::Checkpoint)?;

                                // Publish step failed event
                                event_bus.publish(AgentEvent::WorkflowStepFailed {
                                    run_id,
                                    step_id: step.id.clone(),
                                    step_name: step.name.clone(),
                                    error: err_msg.clone(),
                                    will_retry: false,
                                });

                                Err(ExecutorError::StepFailed {
                                    step_id: step.id.clone(),
                                    error: err_msg,
                                })
                            }
                            Err(_elapsed) => {
                                // Timeout
                                checkpoint
                                    .checkpoint_step_failed(log_id, "step timed out")
                                    .await
                                    .map_err(ExecutorError::Checkpoint)?;

                                // Publish step failed event
                                event_bus.publish(AgentEvent::WorkflowStepFailed {
                                    run_id,
                                    step_id: step.id.clone(),
                                    step_name: step.name.clone(),
                                    error: "step timed out".to_string(),
                                    will_retry: false,
                                });

                                Err(ExecutorError::StepTimeout {
                                    step_id: step.id.clone(),
                                })
                            }
                        }
                    });
                }

                // Collect results from the wave
                while let Some(result) = join_set.join_next().await {
                    let task_result = result.map_err(|e| {
                        ExecutorError::Workflow(WorkflowError::ExecutionError(
                            format!("task join error: {e}"),
                        ))
                    })?;

                    match task_result {
                        Ok((step_id, output)) => {
                            ctx.set_step_output(&step_id, output.to_value())
                                .map_err(ExecutorError::Workflow)?;

                            // Checkpoint the context after each step
                            self.checkpoint
                                .checkpoint_run_status(
                                    run_id,
                                    WorkflowRunStatus::Running,
                                    None,
                                    Some(&ctx.to_json()),
                                )
                                .await
                                .map_err(ExecutorError::Checkpoint)?;
                        }
                        Err(ExecutorError::ApprovalRequired { step_id, prompt }) => {
                            // Pause the workflow for approval
                            self.checkpoint
                                .checkpoint_run_status(
                                    run_id,
                                    WorkflowRunStatus::Paused,
                                    None,
                                    Some(&ctx.to_json()),
                                )
                                .await
                                .map_err(ExecutorError::Checkpoint)?;

                            return Err(ExecutorError::ApprovalRequired { step_id, prompt });
                        }
                        Err(e) => return Err(e),
                    }
                }
            }

            Ok(WorkflowRunStatus::Completed)
        };

        // Apply workflow-level timeout
        tokio::time::timeout(workflow_timeout, execution)
            .await
            .map_err(|_| ExecutorError::WorkflowTimeout)?
    }
}

impl<R: WorkflowRepository + 'static> WorkflowExecutor for DagExecutor<R> {
    async fn execute(
        &self,
        definition: &WorkflowDefinition,
        trigger_type: &str,
        trigger_payload: Option<Value>,
    ) -> Result<ExecutionResult, ExecutorError> {
        // Acquire concurrency permit (released on drop)
        let _permit = self.acquire_concurrency_permit(definition).await?;

        let run_id = Uuid::now_v7();
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancellation_tokens
            .insert(run_id, cancel_token.clone());

        let mut ctx = WorkflowContext::new(
            definition.name.clone(),
            run_id,
            trigger_payload.clone(),
        );

        // Create the run record
        let run = WorkflowRun {
            id: run_id,
            workflow_id: definition.id,
            workflow_name: definition.name.clone(),
            status: WorkflowRunStatus::Running,
            trigger_type: trigger_type.to_string(),
            trigger_payload,
            context: ctx.to_json(),
            started_at: Utc::now(),
            completed_at: None,
            error: None,
            concurrency_key: Some(definition.name.clone()),
        };

        self.checkpoint
            .repo()
            .create_run(&run)
            .await
            .map_err(|e| {
                ExecutorError::Workflow(WorkflowError::ExecutionError(e.to_string()))
            })?;

        // Publish run started event
        self.event_bus.publish(AgentEvent::WorkflowRunStarted {
            run_id,
            workflow_name: definition.name.clone(),
            trigger_type: trigger_type.to_string(),
        });

        tracing::info!(
            run_id = %run_id,
            workflow = definition.name.as_str(),
            "starting workflow execution"
        );

        let run_start = std::time::Instant::now();
        let completed_steps = HashSet::new();
        let result = self
            .execute_waves(definition, run_id, &mut ctx, &completed_steps, &cancel_token)
            .await;

        // Clean up cancellation token
        self.cancellation_tokens.remove(&run_id);

        match result {
            Ok(status) => {
                self.checkpoint
                    .checkpoint_run_status(run_id, status, None, Some(&ctx.to_json()))
                    .await
                    .map_err(ExecutorError::Checkpoint)?;

                let completed = self
                    .checkpoint
                    .get_completed_steps(run_id)
                    .await
                    .unwrap_or_default();

                // Publish run completed event
                self.event_bus.publish(AgentEvent::WorkflowRunCompleted {
                    run_id,
                    workflow_name: definition.name.clone(),
                    duration_ms: run_start.elapsed().as_millis() as u64,
                    steps_completed: completed.len() as u32,
                });

                Ok(ExecutionResult {
                    run_id,
                    status,
                    context: ctx,
                    completed_steps: completed,
                    error: None,
                })
            }
            Err(ExecutorError::ApprovalRequired { step_id, prompt }) => {
                // Workflow is paused -- not a failure
                let completed = self
                    .checkpoint
                    .get_completed_steps(run_id)
                    .await
                    .unwrap_or_default();

                // Publish run paused event
                self.event_bus.publish(AgentEvent::WorkflowRunPaused {
                    run_id,
                    step_id: step_id.clone(),
                    reason: prompt.clone(),
                });

                Ok(ExecutionResult {
                    run_id,
                    status: WorkflowRunStatus::Paused,
                    context: ctx,
                    completed_steps: completed,
                    error: Some(format!(
                        "approval required at step '{}': {}",
                        step_id, prompt
                    )),
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                let _ = self
                    .checkpoint
                    .checkpoint_run_status(
                        run_id,
                        WorkflowRunStatus::Failed,
                        Some(&err_msg),
                        Some(&ctx.to_json()),
                    )
                    .await;

                // Publish run failed event
                self.event_bus.publish(AgentEvent::WorkflowRunFailed {
                    run_id,
                    workflow_name: definition.name.clone(),
                    error: err_msg,
                });

                Err(e)
            }
        }
    }

    async fn resume(
        &self,
        run_id: Uuid,
        definition: &WorkflowDefinition,
    ) -> Result<ExecutionResult, ExecutorError> {
        // Restore context from checkpoint
        let context_json = self
            .checkpoint
            .restore_context(run_id)
            .await
            .map_err(ExecutorError::Checkpoint)?;

        let mut ctx = WorkflowContext::from_json(context_json)
            .map_err(ExecutorError::Workflow)?;

        // Get completed steps to skip
        let completed_ids = self
            .checkpoint
            .get_completed_steps(run_id)
            .await
            .map_err(ExecutorError::Checkpoint)?;
        let completed_steps: HashSet<String> = completed_ids.into_iter().collect();

        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancellation_tokens
            .insert(run_id, cancel_token.clone());

        // Mark as running again
        self.checkpoint
            .checkpoint_run_status(
                run_id,
                WorkflowRunStatus::Running,
                None,
                None,
            )
            .await
            .map_err(ExecutorError::Checkpoint)?;

        tracing::info!(
            run_id = %run_id,
            workflow = definition.name.as_str(),
            skipping = completed_steps.len(),
            "resuming workflow execution"
        );

        let result = self
            .execute_waves(definition, run_id, &mut ctx, &completed_steps, &cancel_token)
            .await;

        // Clean up cancellation token
        self.cancellation_tokens.remove(&run_id);

        match result {
            Ok(status) => {
                self.checkpoint
                    .checkpoint_run_status(run_id, status, None, Some(&ctx.to_json()))
                    .await
                    .map_err(ExecutorError::Checkpoint)?;

                let completed = self
                    .checkpoint
                    .get_completed_steps(run_id)
                    .await
                    .unwrap_or_default();

                Ok(ExecutionResult {
                    run_id,
                    status,
                    context: ctx,
                    completed_steps: completed,
                    error: None,
                })
            }
            Err(ExecutorError::ApprovalRequired { step_id, prompt }) => {
                let completed = self
                    .checkpoint
                    .get_completed_steps(run_id)
                    .await
                    .unwrap_or_default();

                Ok(ExecutionResult {
                    run_id,
                    status: WorkflowRunStatus::Paused,
                    context: ctx,
                    completed_steps: completed,
                    error: Some(format!(
                        "approval required at step '{}': {}",
                        step_id, prompt
                    )),
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                let _ = self
                    .checkpoint
                    .checkpoint_run_status(
                        run_id,
                        WorkflowRunStatus::Failed,
                        Some(&err_msg),
                        Some(&ctx.to_json()),
                    )
                    .await;

                Err(e)
            }
        }
    }

    async fn cancel(
        &self,
        run_id: Uuid,
    ) -> Result<(), ExecutorError> {
        if let Some((_, token)) = self.cancellation_tokens.remove(&run_id) {
            token.cancel();
            self.checkpoint
                .checkpoint_run_status(
                    run_id,
                    WorkflowRunStatus::Cancelled,
                    Some("cancelled by user"),
                    None,
                )
                .await
                .map_err(ExecutorError::Checkpoint)?;

            tracing::info!(run_id = %run_id, "workflow cancelled");
            Ok(())
        } else {
            Err(ExecutorError::RunNotFound(run_id))
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutorError
// ---------------------------------------------------------------------------

/// Errors that can occur during workflow execution.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    /// Workflow-level error (definition, DAG, expression).
    #[error("workflow error: {0}")]
    Workflow(#[from] WorkflowError),

    /// Checkpoint persistence error.
    #[error("checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),

    /// A step failed during execution.
    #[error("step '{step_id}' failed: {error}")]
    StepFailed { step_id: String, error: String },

    /// A step exceeded its timeout.
    #[error("step '{step_id}' timed out")]
    StepTimeout { step_id: String },

    /// Workflow exceeded its overall timeout.
    #[error("workflow timed out")]
    WorkflowTimeout,

    /// An approval gate requires human intervention.
    #[error("approval required at step '{step_id}': {prompt}")]
    ApprovalRequired { step_id: String, prompt: String },

    /// Workflow execution was cancelled.
    #[error("workflow cancelled")]
    Cancelled,

    /// Run not found (for cancel/resume).
    #[error("workflow run not found: {0}")]
    RunNotFound(Uuid),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_error_display() {
        let err = ExecutorError::StepFailed {
            step_id: "gather".to_string(),
            error: "timeout".to_string(),
        };
        assert!(err.to_string().contains("gather"));
        assert!(err.to_string().contains("timeout"));

        let err = ExecutorError::WorkflowTimeout;
        assert!(err.to_string().contains("timed out"));

        let err = ExecutorError::ApprovalRequired {
            step_id: "review".to_string(),
            prompt: "check results".to_string(),
        };
        assert!(err.to_string().contains("review"));
        assert!(err.to_string().contains("check results"));
    }

    #[test]
    fn execution_result_default_fields() {
        let result = ExecutionResult {
            run_id: Uuid::nil(),
            status: WorkflowRunStatus::Completed,
            context: WorkflowContext::new("test".to_string(), Uuid::nil(), None),
            completed_steps: vec!["a".to_string(), "b".to_string()],
            error: None,
        };
        assert_eq!(result.completed_steps.len(), 2);
        assert!(result.error.is_none());
    }
}
