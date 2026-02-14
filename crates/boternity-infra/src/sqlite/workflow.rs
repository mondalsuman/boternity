//! SQLite workflow repository implementation.
//!
//! Implements `WorkflowRepository` from `boternity-core` using sqlx with split
//! read/write pools. Workflow definitions are stored as JSON blobs. Runs and
//! step logs track execution state for crash recovery and auditing.

use boternity_core::repository::workflow::WorkflowRepository;
use boternity_types::error::RepositoryError;
use boternity_types::workflow::{
    WorkflowDefinition, WorkflowOwner, WorkflowRun, WorkflowRunStatus, WorkflowStepLog,
    WorkflowStepStatus,
};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `WorkflowRepository`.
pub struct SqliteWorkflowRepository {
    pool: DatabasePool,
}

impl SqliteWorkflowRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Internal row types
// ---------------------------------------------------------------------------

struct WorkflowDefRow {
    definition: String,
}

impl WorkflowDefRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            definition: row.try_get("definition")?,
        })
    }

    fn into_definition(self) -> Result<WorkflowDefinition, RepositoryError> {
        serde_json::from_str(&self.definition)
            .map_err(|e| RepositoryError::Query(format!("invalid workflow definition JSON: {e}")))
    }
}

struct WorkflowRunRow {
    id: String,
    workflow_id: String,
    workflow_name: String,
    status: String,
    trigger_type: String,
    trigger_payload: Option<String>,
    context: String,
    started_at: String,
    completed_at: Option<String>,
    error: Option<String>,
    concurrency_key: Option<String>,
}

impl WorkflowRunRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            workflow_id: row.try_get("workflow_id")?,
            workflow_name: row.try_get("workflow_name")?,
            status: row.try_get("status")?,
            trigger_type: row.try_get("trigger_type")?,
            trigger_payload: row.try_get("trigger_payload")?,
            context: row.try_get("context")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            error: row.try_get("error")?,
            concurrency_key: row.try_get("concurrency_key")?,
        })
    }

    fn into_run(self) -> Result<WorkflowRun, RepositoryError> {
        let id = parse_uuid(&self.id)?;
        let workflow_id = parse_uuid(&self.workflow_id)?;
        let status: WorkflowRunStatus = serde_json::from_value(
            serde_json::Value::String(self.status.clone()),
        )
        .map_err(|_| RepositoryError::Query(format!("invalid run status: {}", self.status)))?;

        let trigger_payload = self
            .trigger_payload
            .as_deref()
            .map(|s| {
                serde_json::from_str(s)
                    .map_err(|e| RepositoryError::Query(format!("invalid trigger_payload: {e}")))
            })
            .transpose()?;

        let context: serde_json::Value = serde_json::from_str(&self.context)
            .map_err(|e| RepositoryError::Query(format!("invalid context JSON: {e}")))?;

        let started_at = parse_datetime(&self.started_at)?;
        let completed_at = self
            .completed_at
            .as_deref()
            .map(parse_datetime)
            .transpose()?;

        Ok(WorkflowRun {
            id,
            workflow_id,
            workflow_name: self.workflow_name,
            status,
            trigger_type: self.trigger_type,
            trigger_payload,
            context,
            started_at,
            completed_at,
            error: self.error,
            concurrency_key: self.concurrency_key,
        })
    }
}

struct WorkflowStepRow {
    id: String,
    run_id: String,
    step_id: String,
    step_name: String,
    status: String,
    attempt: i32,
    idempotency_key: Option<String>,
    input: Option<String>,
    output: Option<String>,
    error: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
}

impl WorkflowStepRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            run_id: row.try_get("run_id")?,
            step_id: row.try_get("step_id")?,
            step_name: row.try_get("step_name")?,
            status: row.try_get("status")?,
            attempt: row.try_get("attempt")?,
            idempotency_key: row.try_get("idempotency_key")?,
            input: row.try_get("input")?,
            output: row.try_get("output")?,
            error: row.try_get("error")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
        })
    }

    fn into_step_log(self) -> Result<WorkflowStepLog, RepositoryError> {
        let id = parse_uuid(&self.id)?;
        let run_id = parse_uuid(&self.run_id)?;
        let status: WorkflowStepStatus = serde_json::from_value(
            serde_json::Value::String(self.status.clone()),
        )
        .map_err(|_| RepositoryError::Query(format!("invalid step status: {}", self.status)))?;

        let input = self
            .input
            .as_deref()
            .map(|s| {
                serde_json::from_str(s)
                    .map_err(|e| RepositoryError::Query(format!("invalid step input: {e}")))
            })
            .transpose()?;

        let output = self
            .output
            .as_deref()
            .map(|s| {
                serde_json::from_str(s)
                    .map_err(|e| RepositoryError::Query(format!("invalid step output: {e}")))
            })
            .transpose()?;

        let started_at = self
            .started_at
            .as_deref()
            .map(parse_datetime)
            .transpose()?;

        let completed_at = self
            .completed_at
            .as_deref()
            .map(parse_datetime)
            .transpose()?;

        Ok(WorkflowStepLog {
            id,
            run_id,
            step_id: self.step_id,
            step_name: self.step_name,
            status,
            attempt: self.attempt as u32,
            idempotency_key: self.idempotency_key,
            input,
            output,
            error: self.error,
            started_at,
            completed_at,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_uuid(s: &str) -> Result<Uuid, RepositoryError> {
    s.parse::<Uuid>()
        .map_err(|e| RepositoryError::Query(format!("invalid UUID: {e}")))
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Query(format!("invalid datetime: {e}")))
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

/// Extract owner_type and owner_bot_id from a `WorkflowOwner`.
fn owner_parts(owner: &WorkflowOwner) -> (&str, Option<String>) {
    match owner {
        WorkflowOwner::Bot { bot_id, .. } => ("bot", Some(bot_id.to_string())),
        WorkflowOwner::Global => ("global", None),
    }
}

// ---------------------------------------------------------------------------
// WorkflowRepository impl
// ---------------------------------------------------------------------------

impl WorkflowRepository for SqliteWorkflowRepository {
    async fn save_definition(&self, def: &WorkflowDefinition) -> Result<(), RepositoryError> {
        let definition_json = serde_json::to_string(def)
            .map_err(|e| RepositoryError::Query(format!("serialize definition: {e}")))?;

        let (owner_type, owner_bot_id) = owner_parts(&def.owner);
        let now = format_datetime(&Utc::now());

        sqlx::query(
            r#"INSERT INTO workflows (id, name, owner_type, owner_bot_id, definition, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 owner_type = excluded.owner_type,
                 owner_bot_id = excluded.owner_bot_id,
                 definition = excluded.definition,
                 updated_at = excluded.updated_at"#,
        )
        .bind(def.id.to_string())
        .bind(&def.name)
        .bind(owner_type)
        .bind(&owner_bot_id)
        .bind(&definition_json)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_definition(
        &self,
        id: &Uuid,
    ) -> Result<Option<WorkflowDefinition>, RepositoryError> {
        let row = sqlx::query("SELECT id, definition FROM workflows WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let r = WorkflowDefRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(r.into_definition()?))
            }
            None => Ok(None),
        }
    }

    async fn get_definition_by_name(
        &self,
        name: &str,
        owner: &WorkflowOwner,
    ) -> Result<Option<WorkflowDefinition>, RepositoryError> {
        let (owner_type, owner_bot_id) = owner_parts(owner);

        let row = sqlx::query(
            "SELECT id, definition FROM workflows WHERE name = ? AND owner_type = ? AND COALESCE(owner_bot_id, '') = ?",
        )
        .bind(name)
        .bind(owner_type)
        .bind(owner_bot_id.as_deref().unwrap_or(""))
        .fetch_optional(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let r = WorkflowDefRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(r.into_definition()?))
            }
            None => Ok(None),
        }
    }

    async fn list_definitions(
        &self,
        owner: Option<&WorkflowOwner>,
    ) -> Result<Vec<WorkflowDefinition>, RepositoryError> {
        let rows = match owner {
            Some(o) => {
                let (owner_type, owner_bot_id) = owner_parts(o);
                sqlx::query(
                    "SELECT id, definition FROM workflows WHERE owner_type = ? AND COALESCE(owner_bot_id, '') = ? ORDER BY name ASC",
                )
                .bind(owner_type)
                .bind(owner_bot_id.as_deref().unwrap_or(""))
                .fetch_all(&self.pool.reader)
                .await
            }
            None => {
                sqlx::query("SELECT id, definition FROM workflows ORDER BY name ASC")
                    .fetch_all(&self.pool.reader)
                    .await
            }
        }
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut defs = Vec::with_capacity(rows.len());
        for row in &rows {
            let r = WorkflowDefRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            defs.push(r.into_definition()?);
        }
        Ok(defs)
    }

    async fn delete_definition(&self, id: &Uuid) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM workflows WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn create_run(&self, run: &WorkflowRun) -> Result<(), RepositoryError> {
        let status_str = serde_json::to_value(&run.status)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
            .as_str()
            .unwrap_or("pending")
            .to_string();

        let trigger_payload = run
            .trigger_payload
            .as_ref()
            .map(|v| serde_json::to_string(v))
            .transpose()
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let context_str = serde_json::to_string(&run.context)
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO workflow_runs
               (id, workflow_id, workflow_name, status, trigger_type, trigger_payload,
                context, started_at, completed_at, error, concurrency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(run.id.to_string())
        .bind(run.workflow_id.to_string())
        .bind(&run.workflow_name)
        .bind(&status_str)
        .bind(&run.trigger_type)
        .bind(&trigger_payload)
        .bind(&context_str)
        .bind(format_datetime(&run.started_at))
        .bind(run.completed_at.as_ref().map(format_datetime))
        .bind(&run.error)
        .bind(&run.concurrency_key)
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn update_run_status(
        &self,
        run_id: &Uuid,
        status: WorkflowRunStatus,
        error: Option<&str>,
        context: Option<&serde_json::Value>,
    ) -> Result<(), RepositoryError> {
        let status_str = serde_json::to_value(&status)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
            .as_str()
            .unwrap_or("pending")
            .to_string();

        let is_terminal = matches!(
            status,
            WorkflowRunStatus::Completed
                | WorkflowRunStatus::Failed
                | WorkflowRunStatus::Crashed
                | WorkflowRunStatus::Cancelled
        );

        let completed_at = if is_terminal {
            Some(format_datetime(&Utc::now()))
        } else {
            None
        };

        let context_str = context
            .map(|v| serde_json::to_string(v))
            .transpose()
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let result = if let Some(ctx) = &context_str {
            sqlx::query(
                "UPDATE workflow_runs SET status = ?, error = ?, completed_at = COALESCE(?, completed_at), context = ? WHERE id = ?",
            )
            .bind(&status_str)
            .bind(error)
            .bind(&completed_at)
            .bind(ctx)
            .bind(run_id.to_string())
            .execute(&self.pool.writer)
            .await
        } else {
            sqlx::query(
                "UPDATE workflow_runs SET status = ?, error = ?, completed_at = COALESCE(?, completed_at) WHERE id = ?",
            )
            .bind(&status_str)
            .bind(error)
            .bind(&completed_at)
            .bind(run_id.to_string())
            .execute(&self.pool.writer)
            .await
        }
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn get_run(&self, run_id: &Uuid) -> Result<Option<WorkflowRun>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM workflow_runs WHERE id = ?")
            .bind(run_id.to_string())
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let r = WorkflowRunRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(r.into_run()?))
            }
            None => Ok(None),
        }
    }

    async fn list_runs(
        &self,
        workflow_id: &Uuid,
        limit: u32,
    ) -> Result<Vec<WorkflowRun>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM workflow_runs WHERE workflow_id = ? ORDER BY started_at DESC LIMIT ?",
        )
        .bind(workflow_id.to_string())
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in &rows {
            let r = WorkflowRunRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            runs.push(r.into_run()?);
        }
        Ok(runs)
    }

    async fn list_crashed_runs(&self) -> Result<Vec<WorkflowRun>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM workflow_runs WHERE status = 'running' ORDER BY started_at ASC",
        )
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in &rows {
            let r = WorkflowRunRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            runs.push(r.into_run()?);
        }
        Ok(runs)
    }

    async fn create_step_log(&self, step: &WorkflowStepLog) -> Result<(), RepositoryError> {
        let status_str = serde_json::to_value(&step.status)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
            .as_str()
            .unwrap_or("pending")
            .to_string();

        let input_str = step
            .input
            .as_ref()
            .map(|v| serde_json::to_string(v))
            .transpose()
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let output_str = step
            .output
            .as_ref()
            .map(|v| serde_json::to_string(v))
            .transpose()
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO workflow_steps
               (id, run_id, step_id, step_name, status, attempt, idempotency_key,
                input, output, error, started_at, completed_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(step.id.to_string())
        .bind(step.run_id.to_string())
        .bind(&step.step_id)
        .bind(&step.step_name)
        .bind(&status_str)
        .bind(step.attempt as i32)
        .bind(&step.idempotency_key)
        .bind(&input_str)
        .bind(&output_str)
        .bind(&step.error)
        .bind(step.started_at.as_ref().map(format_datetime))
        .bind(step.completed_at.as_ref().map(format_datetime))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn update_step_status(
        &self,
        step_id: &Uuid,
        status: WorkflowStepStatus,
        output: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let status_str = serde_json::to_value(&status)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
            .as_str()
            .unwrap_or("pending")
            .to_string();

        let is_terminal = matches!(
            status,
            WorkflowStepStatus::Completed | WorkflowStepStatus::Failed | WorkflowStepStatus::Skipped
        );

        let completed_at = if is_terminal {
            Some(format_datetime(&Utc::now()))
        } else {
            None
        };

        let output_str = output
            .map(|v| serde_json::to_string(v))
            .transpose()
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let result = sqlx::query(
            "UPDATE workflow_steps SET status = ?, output = COALESCE(?, output), error = COALESCE(?, error), completed_at = COALESCE(?, completed_at) WHERE id = ?",
        )
        .bind(&status_str)
        .bind(&output_str)
        .bind(error)
        .bind(&completed_at)
        .bind(step_id.to_string())
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn list_step_logs(
        &self,
        run_id: &Uuid,
    ) -> Result<Vec<WorkflowStepLog>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM workflow_steps WHERE run_id = ? ORDER BY started_at ASC",
        )
        .bind(run_id.to_string())
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut steps = Vec::with_capacity(rows.len());
        for row in &rows {
            let r = WorkflowStepRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            steps.push(r.into_step_log()?);
        }
        Ok(steps)
    }

    async fn get_completed_step_ids(
        &self,
        run_id: &Uuid,
    ) -> Result<Vec<String>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT step_id FROM workflow_steps WHERE run_id = ? AND status = 'completed' ORDER BY started_at ASC",
        )
        .bind(run_id.to_string())
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            let step_id: String =
                row.try_get("step_id")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
            ids.push(step_id);
        }
        Ok(ids)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::pool::DatabasePool;
    use boternity_types::workflow::*;
    use serde_json::json;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn sample_definition() -> WorkflowDefinition {
        WorkflowDefinition {
            id: Uuid::now_v7(),
            name: "daily-digest".to_string(),
            description: Some("Gather and summarize news".to_string()),
            version: "1.0.0".to_string(),
            owner: WorkflowOwner::Bot {
                bot_id: Uuid::now_v7(),
                slug: "researcher".to_string(),
            },
            concurrency: Some(1),
            timeout_secs: Some(600),
            triggers: vec![TriggerConfig::Manual {}],
            steps: vec![StepDefinition {
                id: "gather".to_string(),
                name: "Gather News".to_string(),
                step_type: StepType::Agent,
                depends_on: vec![],
                condition: None,
                timeout_secs: Some(120),
                retry: None,
                config: StepConfig::Agent {
                    bot: "researcher".to_string(),
                    prompt: "Find top 5 AI news".to_string(),
                    model: None,
                },
                ui: None,
            }],
            metadata: Default::default(),
        }
    }

    fn sample_run(workflow_id: Uuid) -> WorkflowRun {
        WorkflowRun {
            id: Uuid::now_v7(),
            workflow_id,
            workflow_name: "daily-digest".to_string(),
            status: WorkflowRunStatus::Running,
            trigger_type: "manual".to_string(),
            trigger_payload: Some(json!({"user": "test"})),
            context: json!({"steps": {}}),
            started_at: Utc::now(),
            completed_at: None,
            error: None,
            concurrency_key: Some("daily-digest".to_string()),
        }
    }

    fn sample_step_log(run_id: Uuid) -> WorkflowStepLog {
        WorkflowStepLog {
            id: Uuid::now_v7(),
            run_id,
            step_id: "gather".to_string(),
            step_name: "Gather News".to_string(),
            status: WorkflowStepStatus::Running,
            attempt: 1,
            idempotency_key: Some("run-1-gather-1".to_string()),
            input: Some(json!({"query": "AI news"})),
            output: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: None,
        }
    }

    // -- Definition CRUD --

    #[tokio::test]
    async fn test_save_and_get_definition() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();

        repo.save_definition(&def).await.unwrap();

        let loaded = repo.get_definition(&def.id).await.unwrap().unwrap();
        assert_eq!(loaded.name, "daily-digest");
        assert_eq!(loaded.version, "1.0.0");
        assert_eq!(loaded.steps.len(), 1);
    }

    #[tokio::test]
    async fn test_save_definition_upsert() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let mut def = sample_definition();

        repo.save_definition(&def).await.unwrap();

        def.version = "2.0.0".to_string();
        repo.save_definition(&def).await.unwrap();

        let loaded = repo.get_definition(&def.id).await.unwrap().unwrap();
        assert_eq!(loaded.version, "2.0.0");
    }

    #[tokio::test]
    async fn test_get_definition_by_name() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();

        repo.save_definition(&def).await.unwrap();

        let loaded = repo
            .get_definition_by_name("daily-digest", &def.owner)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.id, def.id);
    }

    #[tokio::test]
    async fn test_list_definitions_all() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);

        let mut d1 = sample_definition();
        d1.name = "alpha".to_string();
        let mut d2 = sample_definition();
        d2.id = Uuid::now_v7();
        d2.name = "beta".to_string();
        d2.owner = WorkflowOwner::Global;

        repo.save_definition(&d1).await.unwrap();
        repo.save_definition(&d2).await.unwrap();

        let all = repo.list_definitions(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_list_definitions_by_owner() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);

        let mut d1 = sample_definition();
        d1.name = "owned".to_string();
        let mut d2 = sample_definition();
        d2.id = Uuid::now_v7();
        d2.name = "global".to_string();
        d2.owner = WorkflowOwner::Global;

        repo.save_definition(&d1).await.unwrap();
        repo.save_definition(&d2).await.unwrap();

        let global = repo
            .list_definitions(Some(&WorkflowOwner::Global))
            .await
            .unwrap();
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].name, "global");
    }

    #[tokio::test]
    async fn test_delete_definition() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();

        repo.save_definition(&def).await.unwrap();
        let deleted = repo.delete_definition(&def.id).await.unwrap();
        assert!(deleted);

        let gone = repo.get_definition(&def.id).await.unwrap();
        assert!(gone.is_none());

        let again = repo.delete_definition(&def.id).await.unwrap();
        assert!(!again);
    }

    // -- Run lifecycle --

    #[tokio::test]
    async fn test_create_and_get_run() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();

        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        let loaded = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(loaded.workflow_name, "daily-digest");
        assert_eq!(loaded.status, WorkflowRunStatus::Running);
        assert!(loaded.trigger_payload.is_some());
    }

    #[tokio::test]
    async fn test_update_run_status_completed() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();

        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        let new_ctx = json!({"steps": {"gather": "done"}});
        repo.update_run_status(&run.id, WorkflowRunStatus::Completed, None, Some(&new_ctx))
            .await
            .unwrap();

        let loaded = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, WorkflowRunStatus::Completed);
        assert!(loaded.completed_at.is_some());
        assert_eq!(loaded.context["steps"]["gather"], "done");
    }

    #[tokio::test]
    async fn test_update_run_status_failed() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();

        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        repo.update_run_status(
            &run.id,
            WorkflowRunStatus::Failed,
            Some("step timeout"),
            None,
        )
        .await
        .unwrap();

        let loaded = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, WorkflowRunStatus::Failed);
        assert_eq!(loaded.error.as_deref(), Some("step timeout"));
        assert!(loaded.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_list_runs() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();

        for _ in 0..3 {
            let run = sample_run(def.id);
            repo.create_run(&run).await.unwrap();
        }

        let runs = repo.list_runs(&def.id, 10).await.unwrap();
        assert_eq!(runs.len(), 3);
    }

    #[tokio::test]
    async fn test_list_crashed_runs() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();

        // Create a running run (simulates crash)
        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        // Create a completed run (should not appear)
        let mut completed = sample_run(def.id);
        completed.status = WorkflowRunStatus::Completed;
        completed.completed_at = Some(Utc::now());
        repo.create_run(&completed).await.unwrap();

        let crashed = repo.list_crashed_runs().await.unwrap();
        assert_eq!(crashed.len(), 1);
        assert_eq!(crashed[0].id, run.id);
    }

    // -- Step logs --

    #[tokio::test]
    async fn test_create_and_list_step_logs() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();
        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        let step = sample_step_log(run.id);
        repo.create_step_log(&step).await.unwrap();

        let steps = repo.list_step_logs(&run.id).await.unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_id, "gather");
        assert_eq!(steps[0].status, WorkflowStepStatus::Running);
    }

    #[tokio::test]
    async fn test_update_step_status() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();
        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        let step = sample_step_log(run.id);
        repo.create_step_log(&step).await.unwrap();

        let output = json!({"articles": ["News 1", "News 2"]});
        repo.update_step_status(&step.id, WorkflowStepStatus::Completed, Some(&output), None)
            .await
            .unwrap();

        let steps = repo.list_step_logs(&run.id).await.unwrap();
        assert_eq!(steps[0].status, WorkflowStepStatus::Completed);
        assert!(steps[0].output.is_some());
        assert!(steps[0].completed_at.is_some());
    }

    #[tokio::test]
    async fn test_get_completed_step_ids() {
        let pool = test_pool().await;
        let repo = SqliteWorkflowRepository::new(pool);
        let def = sample_definition();
        repo.save_definition(&def).await.unwrap();
        let run = sample_run(def.id);
        repo.create_run(&run).await.unwrap();

        // Create a completed step
        let mut step1 = sample_step_log(run.id);
        step1.step_id = "gather".to_string();
        step1.status = WorkflowStepStatus::Completed;
        repo.create_step_log(&step1).await.unwrap();

        // Create a running step
        let mut step2 = sample_step_log(run.id);
        step2.id = Uuid::now_v7();
        step2.step_id = "analyze".to_string();
        step2.status = WorkflowStepStatus::Running;
        repo.create_step_log(&step2).await.unwrap();

        let completed = repo.get_completed_step_ids(&run.id).await.unwrap();
        assert_eq!(completed, vec!["gather"]);
    }
}
