//! Trigger manager that coordinates all workflow trigger types.
//!
//! `TriggerManager` is the central registry for workflow triggers. It accepts
//! `TriggerConfig` variants from workflow definitions and routes them to the
//! appropriate subsystem (cron scheduler, webhook registry, event bus listener,
//! or file watcher).
//!
//! Each trigger carries a `TriggerContext` with metadata about the firing event,
//! and an optional `when` clause is evaluated via `WorkflowEvaluator` before
//! the workflow is actually launched.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

use boternity_types::workflow::TriggerConfig;

use super::expression::WorkflowEvaluator;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during trigger operations.
#[derive(Debug, thiserror::Error)]
pub enum TriggerError {
    /// Failed to register a trigger.
    #[error("trigger registration failed: {0}")]
    RegistrationFailed(String),

    /// When-clause evaluation failed.
    #[error("when clause evaluation failed: {0}")]
    WhenClauseError(String),

    /// Unknown workflow referenced by trigger.
    #[error("workflow {0} not found in trigger registry")]
    WorkflowNotFound(Uuid),

    /// Trigger type is unsupported.
    #[error("unsupported trigger type: {0}")]
    Unsupported(String),
}

// ---------------------------------------------------------------------------
// TriggerContext
// ---------------------------------------------------------------------------

/// Metadata about a trigger firing event.
///
/// Passed to the workflow engine when a trigger fires. Contains the trigger
/// type, source, timestamp, and an optional payload (e.g., webhook body,
/// file event, cron metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerContext {
    /// The type of trigger that fired (e.g., "cron", "webhook", "event", "file_watch", "manual").
    pub trigger_type: String,
    /// Source identifier (e.g., cron expression, webhook path, event source).
    pub source: String,
    /// When the trigger fired.
    pub fired_at: DateTime<Utc>,
    /// Optional payload from the trigger (webhook body, file event, etc.).
    pub payload: Option<Value>,
    /// The workflow ID this trigger is associated with.
    pub workflow_id: Uuid,
}

impl TriggerContext {
    /// Create a new trigger context.
    pub fn new(
        trigger_type: impl Into<String>,
        source: impl Into<String>,
        workflow_id: Uuid,
        payload: Option<Value>,
    ) -> Self {
        Self {
            trigger_type: trigger_type.into(),
            source: source.into(),
            fired_at: Utc::now(),
            workflow_id,
            payload,
        }
    }

    /// Build a JSON object suitable for `when` clause evaluation.
    ///
    /// Shape: `{ "trigger": { "type": "...", "source": "...", ... }, "event": <payload> }`
    pub fn to_eval_context(&self) -> Value {
        serde_json::json!({
            "trigger": {
                "type": self.trigger_type,
                "source": self.source,
                "fired_at": self.fired_at.to_rfc3339(),
                "workflow_id": self.workflow_id.to_string(),
            },
            "event": self.payload.clone().unwrap_or(Value::Null),
        })
    }
}

// ---------------------------------------------------------------------------
// TriggerRegistration
// ---------------------------------------------------------------------------

/// A single trigger registration for a workflow.
#[derive(Debug, Clone)]
pub struct TriggerRegistration {
    /// Workflow ID this trigger belongs to.
    pub workflow_id: Uuid,
    /// Workflow name (for logging).
    pub workflow_name: String,
    /// The trigger configuration.
    pub config: TriggerConfig,
}

// ---------------------------------------------------------------------------
// TriggerManager
// ---------------------------------------------------------------------------

/// Central registry and coordinator for all workflow trigger types.
///
/// Manages trigger registrations, evaluates `when` clauses, and routes
/// trigger configs to appropriate subsystems. The actual subsystem integration
/// (cron scheduler, webhook handler, etc.) is handled by infra-layer code that
/// calls into this manager.
pub struct TriggerManager {
    /// All registered triggers indexed by workflow ID.
    registrations: Arc<RwLock<HashMap<Uuid, Vec<TriggerRegistration>>>>,
    /// Expression evaluator for `when` clauses.
    evaluator: WorkflowEvaluator,
}

impl TriggerManager {
    /// Create a new trigger manager.
    pub fn new() -> Self {
        Self {
            registrations: Arc::new(RwLock::new(HashMap::new())),
            evaluator: WorkflowEvaluator::new(),
        }
    }

    /// Register all triggers for a workflow.
    ///
    /// Accepts the full list of `TriggerConfig` from a workflow definition
    /// and routes each to the appropriate internal registry.
    pub async fn register_workflow(
        &self,
        workflow_id: Uuid,
        workflow_name: &str,
        triggers: &[TriggerConfig],
    ) -> Result<(), TriggerError> {
        let mut regs = Vec::new();

        for config in triggers {
            let reg = TriggerRegistration {
                workflow_id,
                workflow_name: workflow_name.to_string(),
                config: config.clone(),
            };

            // Validate trigger config
            Self::validate_trigger_config(config)?;

            regs.push(reg);
        }

        let mut registrations = self.registrations.write().await;
        registrations.insert(workflow_id, regs);

        tracing::info!(
            %workflow_id,
            workflow_name,
            trigger_count = triggers.len(),
            "registered workflow triggers"
        );

        Ok(())
    }

    /// Unregister all triggers for a workflow.
    pub async fn unregister_workflow(&self, workflow_id: Uuid) -> Result<(), TriggerError> {
        let mut registrations = self.registrations.write().await;
        registrations
            .remove(&workflow_id)
            .ok_or(TriggerError::WorkflowNotFound(workflow_id))?;

        tracing::info!(%workflow_id, "unregistered workflow triggers");
        Ok(())
    }

    /// Get all trigger registrations for a workflow.
    pub async fn get_registrations(
        &self,
        workflow_id: Uuid,
    ) -> Option<Vec<TriggerRegistration>> {
        let registrations = self.registrations.read().await;
        registrations.get(&workflow_id).cloned()
    }

    /// Get all registered cron triggers across all workflows.
    ///
    /// Returns `(workflow_id, schedule_expression)` pairs.
    pub async fn get_cron_triggers(&self) -> Vec<(Uuid, String)> {
        let registrations = self.registrations.read().await;
        let mut result = Vec::new();

        for (wf_id, regs) in registrations.iter() {
            for reg in regs {
                if let TriggerConfig::Cron { schedule, .. } = &reg.config {
                    result.push((*wf_id, schedule.clone()));
                }
            }
        }

        result
    }

    /// Get all registered webhook triggers across all workflows.
    ///
    /// Returns `(workflow_id, path)` pairs.
    pub async fn get_webhook_triggers(&self) -> Vec<(Uuid, String)> {
        let registrations = self.registrations.read().await;
        let mut result = Vec::new();

        for (wf_id, regs) in registrations.iter() {
            for reg in regs {
                if let TriggerConfig::Webhook { path, .. } = &reg.config {
                    result.push((*wf_id, path.clone()));
                }
            }
        }

        result
    }

    /// Get all registered event triggers across all workflows.
    pub async fn get_event_triggers(&self) -> Vec<(Uuid, String, String, Option<String>)> {
        let registrations = self.registrations.read().await;
        let mut result = Vec::new();

        for (wf_id, regs) in registrations.iter() {
            for reg in regs {
                if let TriggerConfig::Event {
                    source,
                    event_type,
                    when,
                } = &reg.config
                {
                    result.push((
                        *wf_id,
                        source.clone(),
                        event_type.clone(),
                        when.clone(),
                    ));
                }
            }
        }

        result
    }

    /// Get all registered file watch triggers across all workflows.
    pub async fn get_file_watch_triggers(&self) -> Vec<(Uuid, Vec<String>, Option<Vec<String>>)> {
        let registrations = self.registrations.read().await;
        let mut result = Vec::new();

        for (wf_id, regs) in registrations.iter() {
            for reg in regs {
                if let TriggerConfig::FileWatch {
                    paths, patterns, ..
                } = &reg.config
                {
                    result.push((*wf_id, paths.clone(), patterns.clone()));
                }
            }
        }

        result
    }

    /// Evaluate a `when` clause against a trigger context.
    ///
    /// Returns `true` if no `when` clause is set, or if the clause evaluates
    /// to true. Returns `false` if the clause evaluates to false (the trigger
    /// should be suppressed).
    pub fn evaluate_when_clause(
        &self,
        when: Option<&str>,
        trigger_ctx: &TriggerContext,
    ) -> Result<bool, TriggerError> {
        match when {
            None => Ok(true), // No filter, always passes
            Some(expr) => {
                let eval_ctx = trigger_ctx.to_eval_context();
                self.evaluator
                    .evaluate_bool(expr, &eval_ctx)
                    .map_err(|e| TriggerError::WhenClauseError(e.to_string()))
            }
        }
    }

    /// Get the total number of registered workflows.
    pub async fn workflow_count(&self) -> usize {
        self.registrations.read().await.len()
    }

    /// Get the total number of individual trigger registrations.
    pub async fn trigger_count(&self) -> usize {
        self.registrations
            .read()
            .await
            .values()
            .map(|v| v.len())
            .sum()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Validate a trigger configuration.
    fn validate_trigger_config(config: &TriggerConfig) -> Result<(), TriggerError> {
        match config {
            TriggerConfig::Cron { schedule, .. } => {
                // Validate that the schedule can be normalized
                super::scheduler::normalize_schedule(schedule).map_err(|e| {
                    TriggerError::RegistrationFailed(format!(
                        "invalid cron schedule '{schedule}': {e}"
                    ))
                })?;
                Ok(())
            }
            TriggerConfig::Webhook { path, .. } => {
                if path.is_empty() {
                    return Err(TriggerError::RegistrationFailed(
                        "webhook path must not be empty".to_string(),
                    ));
                }
                if !path.starts_with('/') {
                    return Err(TriggerError::RegistrationFailed(format!(
                        "webhook path must start with '/': '{path}'"
                    )));
                }
                Ok(())
            }
            TriggerConfig::Event {
                source,
                event_type,
                ..
            } => {
                if source.is_empty() {
                    return Err(TriggerError::RegistrationFailed(
                        "event source must not be empty".to_string(),
                    ));
                }
                if event_type.is_empty() {
                    return Err(TriggerError::RegistrationFailed(
                        "event type must not be empty".to_string(),
                    ));
                }
                Ok(())
            }
            TriggerConfig::FileWatch { paths, .. } => {
                if paths.is_empty() {
                    return Err(TriggerError::RegistrationFailed(
                        "file watch must have at least one path".to_string(),
                    ));
                }
                Ok(())
            }
            TriggerConfig::Manual {} => Ok(()),
        }
    }
}

impl Default for TriggerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::workflow::{TriggerConfig, WebhookAuth};
    use serde_json::json;

    // -------------------------------------------------------------------
    // TriggerContext
    // -------------------------------------------------------------------

    #[test]
    fn test_trigger_context_new() {
        let wf_id = Uuid::now_v7();
        let ctx = TriggerContext::new("cron", "0 9 * * *", wf_id, None);
        assert_eq!(ctx.trigger_type, "cron");
        assert_eq!(ctx.source, "0 9 * * *");
        assert_eq!(ctx.workflow_id, wf_id);
        assert!(ctx.payload.is_none());
    }

    #[test]
    fn test_trigger_context_to_eval_context() {
        let wf_id = Uuid::now_v7();
        let payload = json!({ "source": "github", "action": "push" });
        let ctx = TriggerContext::new("webhook", "/hook", wf_id, Some(payload));

        let eval = ctx.to_eval_context();
        assert_eq!(eval["trigger"]["type"], json!("webhook"));
        assert_eq!(eval["trigger"]["source"], json!("/hook"));
        assert_eq!(eval["event"]["source"], json!("github"));
        assert_eq!(eval["event"]["action"], json!("push"));
    }

    #[test]
    fn test_trigger_context_serialization_roundtrip() {
        let wf_id = Uuid::now_v7();
        let ctx = TriggerContext::new("event", "internal", wf_id, Some(json!({"key": "val"})));
        let json_str = serde_json::to_string(&ctx).unwrap();
        let parsed: TriggerContext = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.trigger_type, "event");
        assert_eq!(parsed.source, "internal");
        assert_eq!(parsed.workflow_id, wf_id);
    }

    // -------------------------------------------------------------------
    // TriggerManager registration
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_register_and_unregister_workflow() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        let triggers = vec![
            TriggerConfig::Manual {},
            TriggerConfig::Cron {
                schedule: "every 5 minutes".to_string(),
                timezone: None,
            },
        ];

        mgr.register_workflow(wf_id, "test-wf", &triggers)
            .await
            .unwrap();
        assert_eq!(mgr.workflow_count().await, 1);
        assert_eq!(mgr.trigger_count().await, 2);

        mgr.unregister_workflow(wf_id).await.unwrap();
        assert_eq!(mgr.workflow_count().await, 0);
    }

    #[tokio::test]
    async fn test_register_validates_cron_schedule() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        let triggers = vec![TriggerConfig::Cron {
            schedule: "invalid cron".to_string(),
            timezone: None,
        }];

        let result = mgr.register_workflow(wf_id, "test-wf", &triggers).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_validates_webhook_path() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        // Empty path
        let triggers = vec![TriggerConfig::Webhook {
            path: "".to_string(),
            auth: None,
            when: None,
        }];
        assert!(
            mgr.register_workflow(wf_id, "test-wf", &triggers)
                .await
                .is_err()
        );

        // Path without leading slash
        let triggers = vec![TriggerConfig::Webhook {
            path: "hook".to_string(),
            auth: None,
            when: None,
        }];
        assert!(
            mgr.register_workflow(wf_id, "test-wf", &triggers)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_register_validates_event_trigger() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        // Empty source
        let triggers = vec![TriggerConfig::Event {
            source: "".to_string(),
            event_type: "push".to_string(),
            when: None,
        }];
        assert!(
            mgr.register_workflow(wf_id, "test-wf", &triggers)
                .await
                .is_err()
        );

        // Empty event_type
        let triggers = vec![TriggerConfig::Event {
            source: "github".to_string(),
            event_type: "".to_string(),
            when: None,
        }];
        assert!(
            mgr.register_workflow(wf_id, "test-wf", &triggers)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_register_validates_file_watch() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        // Empty paths
        let triggers = vec![TriggerConfig::FileWatch {
            paths: vec![],
            patterns: None,
            when: None,
        }];
        assert!(
            mgr.register_workflow(wf_id, "test-wf", &triggers)
                .await
                .is_err()
        );
    }

    // -------------------------------------------------------------------
    // Trigger listing
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_cron_triggers() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        let triggers = vec![
            TriggerConfig::Cron {
                schedule: "0 9 * * *".to_string(),
                timezone: None,
            },
            TriggerConfig::Manual {},
        ];

        mgr.register_workflow(wf_id, "test-wf", &triggers)
            .await
            .unwrap();

        let crons = mgr.get_cron_triggers().await;
        assert_eq!(crons.len(), 1);
        assert_eq!(crons[0].0, wf_id);
        assert_eq!(crons[0].1, "0 9 * * *");
    }

    #[tokio::test]
    async fn test_get_webhook_triggers() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        let triggers = vec![TriggerConfig::Webhook {
            path: "/trigger/test".to_string(),
            auth: Some(WebhookAuth::BearerToken {
                secret_name: "TOKEN".to_string(),
            }),
            when: Some("event.type == 'push'".to_string()),
        }];

        mgr.register_workflow(wf_id, "test-wf", &triggers)
            .await
            .unwrap();

        let webhooks = mgr.get_webhook_triggers().await;
        assert_eq!(webhooks.len(), 1);
        assert_eq!(webhooks[0].1, "/trigger/test");
    }

    #[tokio::test]
    async fn test_get_event_triggers() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        let triggers = vec![TriggerConfig::Event {
            source: "internal".to_string(),
            event_type: "bot_created".to_string(),
            when: Some("event.name|length > 0".to_string()),
        }];

        mgr.register_workflow(wf_id, "test-wf", &triggers)
            .await
            .unwrap();

        let events = mgr.get_event_triggers().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, "internal");
        assert_eq!(events[0].2, "bot_created");
        assert_eq!(events[0].3.as_deref(), Some("event.name|length > 0"));
    }

    #[tokio::test]
    async fn test_get_file_watch_triggers() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();

        let triggers = vec![TriggerConfig::FileWatch {
            paths: vec!["/data".to_string(), "/logs".to_string()],
            patterns: Some(vec!["*.csv".to_string()]),
            when: None,
        }];

        mgr.register_workflow(wf_id, "test-wf", &triggers)
            .await
            .unwrap();

        let watches = mgr.get_file_watch_triggers().await;
        assert_eq!(watches.len(), 1);
        assert_eq!(watches[0].1.len(), 2);
        assert_eq!(watches[0].2, Some(vec!["*.csv".to_string()]));
    }

    // -------------------------------------------------------------------
    // When clause evaluation
    // -------------------------------------------------------------------

    #[test]
    fn test_when_clause_none_passes() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();
        let ctx = TriggerContext::new("webhook", "/hook", wf_id, None);

        assert!(mgr.evaluate_when_clause(None, &ctx).unwrap());
    }

    #[test]
    fn test_when_clause_matching() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();
        let payload = json!({ "source": "github", "action": "push" });
        let ctx = TriggerContext::new("webhook", "/hook", wf_id, Some(payload));

        assert!(mgr
            .evaluate_when_clause(Some("event.source == 'github'"), &ctx)
            .unwrap());
    }

    #[test]
    fn test_when_clause_not_matching() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();
        let payload = json!({ "source": "gitlab", "action": "push" });
        let ctx = TriggerContext::new("webhook", "/hook", wf_id, Some(payload));

        assert!(!mgr
            .evaluate_when_clause(Some("event.source == 'github'"), &ctx)
            .unwrap());
    }

    #[test]
    fn test_when_clause_complex_expression() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();
        let payload = json!({
            "source": "github",
            "action": "push",
            "branch": "main"
        });
        let ctx = TriggerContext::new("webhook", "/hook", wf_id, Some(payload));

        assert!(mgr
            .evaluate_when_clause(
                Some("event.source == 'github' && event.branch == 'main'"),
                &ctx
            )
            .unwrap());

        assert!(!mgr
            .evaluate_when_clause(
                Some("event.source == 'github' && event.branch == 'develop'"),
                &ctx
            )
            .unwrap());
    }

    #[test]
    fn test_when_clause_with_transforms() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();
        let payload = json!({ "message": "  CRITICAL ERROR  " });
        let ctx = TriggerContext::new("event", "alerts", wf_id, Some(payload));

        assert!(mgr
            .evaluate_when_clause(Some("event.message|trim|lower|contains('critical')"), &ctx)
            .unwrap());
    }

    #[test]
    fn test_when_clause_invalid_expression() {
        let mgr = TriggerManager::new();
        let wf_id = Uuid::now_v7();
        let ctx = TriggerContext::new("webhook", "/hook", wf_id, None);

        // Invalid expression should return an error, not panic
        let result = mgr.evaluate_when_clause(Some("<<<invalid>>>"), &ctx);
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------
    // Unregister unknown workflow
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_unregister_unknown_workflow_fails() {
        let mgr = TriggerManager::new();
        let result = mgr.unregister_workflow(Uuid::now_v7()).await;
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------
    // Multiple workflows
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_multiple_workflows_registered() {
        let mgr = TriggerManager::new();

        let wf1 = Uuid::now_v7();
        let wf2 = Uuid::now_v7();

        mgr.register_workflow(
            wf1,
            "wf-one",
            &[
                TriggerConfig::Cron {
                    schedule: "every minute".to_string(),
                    timezone: None,
                },
                TriggerConfig::Manual {},
            ],
        )
        .await
        .unwrap();

        mgr.register_workflow(
            wf2,
            "wf-two",
            &[TriggerConfig::Webhook {
                path: "/trigger/two".to_string(),
                auth: None,
                when: None,
            }],
        )
        .await
        .unwrap();

        assert_eq!(mgr.workflow_count().await, 2);
        assert_eq!(mgr.trigger_count().await, 3);

        let crons = mgr.get_cron_triggers().await;
        assert_eq!(crons.len(), 1);

        let webhooks = mgr.get_webhook_triggers().await;
        assert_eq!(webhooks.len(), 1);
    }
}
