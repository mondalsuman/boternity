//! Durable checkpoint manager for workflow execution state.
//!
//! Wraps `WorkflowRepository` to provide a higher-level API for recording
//! step-level execution checkpoints. Each step transition (pending -> running
//! -> completed/failed/skipped) is persisted to SQLite so that crashed
//! workflows can resume from the last completed step.

use boternity_types::workflow::{WorkflowRunStatus, WorkflowStepLog, WorkflowStepStatus};
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::repository::workflow::WorkflowRepository;

// ---------------------------------------------------------------------------
// CheckpointManager
// ---------------------------------------------------------------------------

/// Manages durable execution checkpoints for workflow runs.
///
/// Generic over `R: WorkflowRepository` so it works with any storage backend
/// (SQLite, in-memory mock, etc.). Every state transition is persisted before
/// the executor moves forward, ensuring crash recoverability.
pub struct CheckpointManager<R: WorkflowRepository> {
    repo: R,
}

impl<R: WorkflowRepository> CheckpointManager<R> {
    /// Create a new checkpoint manager backed by the given repository.
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Access the underlying repository.
    pub fn repo(&self) -> &R {
        &self.repo
    }

    // -----------------------------------------------------------------------
    // Step-level checkpoints
    // -----------------------------------------------------------------------

    /// Checkpoint a step as starting (Pending -> Running).
    ///
    /// Creates the step log entry and marks it as `Running`.
    pub async fn checkpoint_step_start(
        &self,
        run_id: Uuid,
        step_id: &str,
        step_name: &str,
        attempt: u32,
    ) -> Result<Uuid, CheckpointError> {
        let log_id = Uuid::now_v7();
        let log = WorkflowStepLog {
            id: log_id,
            run_id,
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            status: WorkflowStepStatus::Running,
            attempt,
            idempotency_key: Some(format!("{run_id}-{step_id}-{attempt}")),
            input: None,
            output: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: None,
        };

        self.repo
            .create_step_log(&log)
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?;

        tracing::debug!(
            run_id = %run_id,
            step_id,
            log_id = %log_id,
            "checkpointed step start"
        );

        Ok(log_id)
    }

    /// Checkpoint a step as completed successfully.
    pub async fn checkpoint_step_complete(
        &self,
        log_id: Uuid,
        output: Option<&Value>,
    ) -> Result<(), CheckpointError> {
        self.repo
            .update_step_status(
                &log_id,
                WorkflowStepStatus::Completed,
                output,
                None,
            )
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?;

        tracing::debug!(log_id = %log_id, "checkpointed step complete");
        Ok(())
    }

    /// Checkpoint a step as failed.
    pub async fn checkpoint_step_failed(
        &self,
        log_id: Uuid,
        error: &str,
    ) -> Result<(), CheckpointError> {
        self.repo
            .update_step_status(
                &log_id,
                WorkflowStepStatus::Failed,
                None,
                Some(error),
            )
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?;

        tracing::debug!(log_id = %log_id, error, "checkpointed step failed");
        Ok(())
    }

    /// Checkpoint a step as skipped (condition not met).
    pub async fn checkpoint_step_skipped(
        &self,
        run_id: Uuid,
        step_id: &str,
        step_name: &str,
    ) -> Result<(), CheckpointError> {
        let log = WorkflowStepLog {
            id: Uuid::now_v7(),
            run_id,
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            status: WorkflowStepStatus::Skipped,
            attempt: 0,
            idempotency_key: None,
            input: None,
            output: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
        };

        self.repo
            .create_step_log(&log)
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?;

        tracing::debug!(run_id = %run_id, step_id, "checkpointed step skipped");
        Ok(())
    }

    /// Checkpoint a step as waiting for approval.
    pub async fn checkpoint_step_waiting_approval(
        &self,
        log_id: Uuid,
    ) -> Result<(), CheckpointError> {
        self.repo
            .update_step_status(
                &log_id,
                WorkflowStepStatus::WaitingApproval,
                None,
                None,
            )
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?;

        tracing::debug!(log_id = %log_id, "checkpointed step waiting approval");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Run-level checkpoints
    // -----------------------------------------------------------------------

    /// Update the overall run status and optionally the context snapshot.
    pub async fn checkpoint_run_status(
        &self,
        run_id: Uuid,
        status: WorkflowRunStatus,
        error: Option<&str>,
        context: Option<&Value>,
    ) -> Result<(), CheckpointError> {
        self.repo
            .update_run_status(&run_id, status, error, context)
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?;

        tracing::debug!(
            run_id = %run_id,
            status = ?status,
            "checkpointed run status"
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Recovery helpers
    // -----------------------------------------------------------------------

    /// Get the set of step IDs that completed successfully in a run.
    ///
    /// Used during crash recovery to determine which steps to skip.
    pub async fn get_completed_steps(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<String>, CheckpointError> {
        self.repo
            .get_completed_step_ids(&run_id)
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))
    }

    /// Restore the workflow context from a persisted run.
    ///
    /// Returns the context JSON stored in the run record.
    pub async fn restore_context(
        &self,
        run_id: Uuid,
    ) -> Result<Value, CheckpointError> {
        let run = self
            .repo
            .get_run(&run_id)
            .await
            .map_err(|e| CheckpointError::Repository(e.to_string()))?
            .ok_or(CheckpointError::RunNotFound(run_id))?;

        Ok(run.context)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during checkpoint operations.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    /// Underlying repository operation failed.
    #[error("checkpoint repository error: {0}")]
    Repository(String),

    /// Workflow run not found (for restore operations).
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
    fn checkpoint_error_display() {
        let err = CheckpointError::Repository("connection lost".to_string());
        assert!(err.to_string().contains("connection lost"));

        let err = CheckpointError::RunNotFound(Uuid::nil());
        assert!(err.to_string().contains("not found"));
    }
}
