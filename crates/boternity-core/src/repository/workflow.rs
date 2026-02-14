//! Workflow repository trait definition.
//!
//! Defines the storage interface for workflow definitions, execution runs,
//! and step logs. The infrastructure layer (boternity-infra) implements
//! this trait with SQLite persistence.

use boternity_types::error::RepositoryError;
use boternity_types::workflow::{
    WorkflowDefinition, WorkflowOwner, WorkflowRun, WorkflowRunStatus, WorkflowStepLog,
    WorkflowStepStatus,
};
use uuid::Uuid;

/// Repository trait for workflow persistence.
///
/// Covers three entity families:
/// - **Definitions:** CRUD for workflow definitions (the canonical IR).
/// - **Runs:** Create/update/query workflow execution instances.
/// - **Steps:** Create/update/query individual step execution logs.
///
/// Uses native async fn in traits (Rust 2024 edition, no async_trait macro).
pub trait WorkflowRepository: Send + Sync {
    // -----------------------------------------------------------------------
    // Definitions
    // -----------------------------------------------------------------------

    /// Upsert a workflow definition (insert or replace by ID).
    fn save_definition(
        &self,
        def: &WorkflowDefinition,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get a workflow definition by its UUID.
    fn get_definition(
        &self,
        id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Option<WorkflowDefinition>, RepositoryError>> + Send;

    /// Get a workflow definition by name and owner.
    fn get_definition_by_name(
        &self,
        name: &str,
        owner: &WorkflowOwner,
    ) -> impl std::future::Future<Output = Result<Option<WorkflowDefinition>, RepositoryError>> + Send;

    /// List workflow definitions, optionally filtered by owner.
    fn list_definitions(
        &self,
        owner: Option<&WorkflowOwner>,
    ) -> impl std::future::Future<Output = Result<Vec<WorkflowDefinition>, RepositoryError>> + Send;

    /// Delete a workflow definition by ID. Returns `true` if it existed.
    fn delete_definition(
        &self,
        id: &Uuid,
    ) -> impl std::future::Future<Output = Result<bool, RepositoryError>> + Send;

    // -----------------------------------------------------------------------
    // Runs
    // -----------------------------------------------------------------------

    /// Create a new workflow run record.
    fn create_run(
        &self,
        run: &WorkflowRun,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Update a run's status (and optionally error message / context).
    fn update_run_status(
        &self,
        run_id: &Uuid,
        status: WorkflowRunStatus,
        error: Option<&str>,
        context: Option<&serde_json::Value>,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get a workflow run by its UUID.
    fn get_run(
        &self,
        run_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Option<WorkflowRun>, RepositoryError>> + Send;

    /// List runs for a given workflow definition, ordered by started_at DESC.
    fn list_runs(
        &self,
        workflow_id: &Uuid,
        limit: u32,
    ) -> impl std::future::Future<Output = Result<Vec<WorkflowRun>, RepositoryError>> + Send;

    /// List runs that were left in `Running` status (crash recovery).
    fn list_crashed_runs(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<WorkflowRun>, RepositoryError>> + Send;

    // -----------------------------------------------------------------------
    // Steps
    // -----------------------------------------------------------------------

    /// Create a new step execution log entry.
    fn create_step_log(
        &self,
        step: &WorkflowStepLog,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Update a step's status and optionally its output/error.
    fn update_step_status(
        &self,
        step_id: &Uuid,
        status: WorkflowStepStatus,
        output: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// List all step logs for a given run, ordered by started_at ASC.
    fn list_step_logs(
        &self,
        run_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<WorkflowStepLog>, RepositoryError>> + Send;

    /// Get the step IDs that completed successfully in a run (for resume).
    fn get_completed_step_ids(
        &self,
        run_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<String>, RepositoryError>> + Send;
}
