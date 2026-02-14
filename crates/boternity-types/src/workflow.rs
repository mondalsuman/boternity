//! Workflow domain types for Boternity.
//!
//! Defines the canonical intermediate representation for workflows: all three
//! representations (YAML, visual React Flow canvas, programmatic SDK) convert
//! to and from `WorkflowDefinition`. This module also contains execution
//! tracking types (`WorkflowRun`, `WorkflowStepLog`) and trigger configuration.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Workflow Definition (canonical IR)
// ---------------------------------------------------------------------------

/// The canonical workflow definition.
///
/// YAML files, the visual builder, and the SDK all convert to/from this struct.
/// It is the single source of truth for a workflow's shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// UUIDv7 assigned on first save.
    pub id: Uuid,
    /// Human-readable workflow name.
    pub name: String,
    /// Optional longer description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic version string (e.g. "1.0.0").
    pub version: String,
    /// Who owns this workflow.
    pub owner: WorkflowOwner,
    /// Maximum concurrent instances of this workflow (None = unlimited).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<u32>,
    /// Per-workflow timeout in seconds (overrides global default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Trigger configurations (cron, webhook, event, file_watch, manual).
    #[serde(default)]
    pub triggers: Vec<TriggerConfig>,
    /// Ordered list of step definitions forming the workflow DAG.
    pub steps: Vec<StepDefinition>,
    /// Extensible metadata (for future use / custom integrations).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Who owns a workflow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowOwner {
    /// Owned by a specific bot.
    Bot { bot_id: Uuid, slug: String },
    /// Global (cross-bot) workflow.
    Global,
}

// ---------------------------------------------------------------------------
// Step Definition
// ---------------------------------------------------------------------------

/// A single step in the workflow DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDefinition {
    /// User-defined step ID (e.g. "gather-news"). Unique within a workflow.
    pub id: String,
    /// Human-readable step name.
    pub name: String,
    /// The kind of step.
    #[serde(rename = "type")]
    pub step_type: StepType,
    /// Step IDs this step depends on (DAG edges).
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Optional JEXL expression for conditional execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Step-level timeout in seconds (default 300).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Retry configuration for this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryConfig>,
    /// Step-specific configuration payload.
    pub config: StepConfig,
    /// Visual builder metadata (position, group). Omitted from YAML when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<StepUiMetadata>,
}

/// The kind of step in a workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Agent,
    Skill,
    Code,
    Http,
    Conditional,
    Loop,
    Approval,
    SubWorkflow,
}

/// Step-specific configuration payload.
///
/// Internally tagged by `type` to match YAML structure:
/// ```yaml
/// config:
///   type: agent
///   bot: researcher
///   prompt: "Find top news"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepConfig {
    /// Invoke a bot agent with a prompt.
    Agent {
        bot: String,
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
    /// Run an installed skill.
    Skill {
        skill: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<String>,
    },
    /// Execute inline code (TypeScript or WASM).
    Code {
        language: CodeLanguage,
        source: String,
    },
    /// Make an HTTP request.
    Http {
        method: String,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    },
    /// Conditional branching (if/else).
    Conditional {
        condition: String,
        then_steps: Vec<String>,
        else_steps: Vec<String>,
    },
    /// Loop until condition becomes false.
    Loop {
        condition: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_iterations: Option<u32>,
        body_steps: Vec<String>,
    },
    /// Human approval gate.
    Approval {
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
    /// Invoke another workflow by name.
    SubWorkflow {
        workflow_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<serde_json::Value>,
    },
}

/// Language for inline Code steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeLanguage {
    TypeScript,
    Wasm,
}

// ---------------------------------------------------------------------------
// Retry Configuration
// ---------------------------------------------------------------------------

/// Retry configuration for a workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of attempts (default 3).
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    /// Retry strategy.
    pub strategy: RetryStrategy,
}

fn default_max_attempts() -> u32 {
    3
}

/// Strategy for retrying a failed step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryStrategy {
    /// Simple retry (re-execute with same input).
    Simple,
    /// LLM-driven self-correction: feed error back to agent for correction.
    LlmSelfCorrect,
}

// ---------------------------------------------------------------------------
// Trigger Configuration
// ---------------------------------------------------------------------------

/// How a workflow can be triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerConfig {
    /// Manually triggered via CLI or API.
    Manual {},
    /// Cron schedule trigger.
    Cron {
        /// Cron expression or human-readable schedule string.
        schedule: String,
        /// Optional timezone (e.g. "America/New_York").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timezone: Option<String>,
    },
    /// Incoming webhook trigger.
    Webhook {
        /// Webhook endpoint path (e.g. "/trigger/daily-digest").
        path: String,
        /// Optional authentication configuration.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth: Option<WebhookAuth>,
        /// Optional JEXL expression to filter trigger payloads.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<String>,
    },
    /// Internal event bus trigger.
    Event {
        /// Event source identifier.
        source: String,
        /// Event type to match.
        event_type: String,
        /// Optional JEXL filter expression.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<String>,
    },
    /// Filesystem change trigger.
    FileWatch {
        /// Paths to watch.
        paths: Vec<String>,
        /// Optional glob patterns to filter events.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        patterns: Option<Vec<String>>,
        /// Optional JEXL filter expression.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<String>,
    },
}

/// Authentication configuration for webhook triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebhookAuth {
    /// HMAC-SHA256 signature verification.
    HmacSha256 {
        /// Name of the secret in the bot's secret store.
        secret_name: String,
    },
    /// Bearer token verification.
    BearerToken {
        /// Name of the secret in the bot's secret store.
        secret_name: String,
    },
}

// ---------------------------------------------------------------------------
// Visual Builder Metadata
// ---------------------------------------------------------------------------

/// Metadata for the visual workflow builder (React Flow canvas).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepUiMetadata {
    /// Position on the canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<UiPosition>,
    /// Group node ID (for collapsible groups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Whether the node is collapsed in the visual builder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
}

/// Canvas position coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UiPosition {
    pub x: f64,
    pub y: f64,
}

// ---------------------------------------------------------------------------
// Workflow Execution Status
// ---------------------------------------------------------------------------

/// Overall status of a workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Crashed,
    Cancelled,
}

/// Status of an individual workflow step execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    WaitingApproval,
}

// ---------------------------------------------------------------------------
// Workflow Run (query result / audit record)
// ---------------------------------------------------------------------------

/// A single execution instance of a workflow. Used for query results and audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    /// UUIDv7 run ID.
    pub id: Uuid,
    /// ID of the workflow definition being executed.
    pub workflow_id: Uuid,
    /// Name of the workflow (denormalized for display).
    pub workflow_name: String,
    /// Current run status.
    pub status: WorkflowRunStatus,
    /// How this run was triggered (e.g. "manual", "cron", "webhook").
    pub trigger_type: String,
    /// JSON payload from the trigger (e.g. webhook body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_payload: Option<serde_json::Value>,
    /// JSON workflow context (accumulated step outputs).
    pub context: serde_json::Value,
    /// When the run started.
    pub started_at: DateTime<Utc>,
    /// When the run completed (None if still running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message if the run failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Key for concurrency limiting (matches `WorkflowDefinition.name` by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency_key: Option<String>,
}

/// Execution log for a single step within a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepLog {
    /// UUIDv7 step execution ID.
    pub id: Uuid,
    /// Parent workflow run ID.
    pub run_id: Uuid,
    /// Step ID matching `StepDefinition.id`.
    pub step_id: String,
    /// Step name (denormalized for display).
    pub step_name: String,
    /// Current step status.
    pub status: WorkflowStepStatus,
    /// Attempt number (1-based, increments on retry).
    pub attempt: u32,
    /// Idempotency key for side-effecting steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// JSON input passed to this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// JSON output produced by this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// Error message if the step failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When step execution started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// When step execution completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Build a full `WorkflowDefinition` exercising all step and trigger types.
    fn sample_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            id: Uuid::now_v7(),
            name: "daily-digest".to_string(),
            description: Some("Gather news, analyze, summarize".to_string()),
            version: "1.0.0".to_string(),
            owner: WorkflowOwner::Bot {
                bot_id: Uuid::now_v7(),
                slug: "researcher".to_string(),
            },
            concurrency: Some(1),
            timeout_secs: Some(600),
            triggers: vec![
                TriggerConfig::Manual {},
                TriggerConfig::Cron {
                    schedule: "0 9 * * *".to_string(),
                    timezone: Some("America/New_York".to_string()),
                },
                TriggerConfig::Webhook {
                    path: "/trigger/daily-digest".to_string(),
                    auth: Some(WebhookAuth::HmacSha256 {
                        secret_name: "DIGEST_WEBHOOK_SECRET".to_string(),
                    }),
                    when: Some("event.source == 'github'".to_string()),
                },
                TriggerConfig::Event {
                    source: "internal".to_string(),
                    event_type: "bot_created".to_string(),
                    when: None,
                },
                TriggerConfig::FileWatch {
                    paths: vec!["/tmp/data".to_string()],
                    patterns: Some(vec!["*.csv".to_string()]),
                    when: None,
                },
            ],
            steps: vec![
                StepDefinition {
                    id: "gather-news".to_string(),
                    name: "Gather News".to_string(),
                    step_type: StepType::Agent,
                    depends_on: vec![],
                    condition: None,
                    timeout_secs: Some(120),
                    retry: None,
                    config: StepConfig::Agent {
                        bot: "researcher".to_string(),
                        prompt: "Find top 5 AI news stories".to_string(),
                        model: None,
                    },
                    ui: Some(StepUiMetadata {
                        position: Some(UiPosition { x: 100.0, y: 50.0 }),
                        group: None,
                        collapsed: None,
                    }),
                },
                StepDefinition {
                    id: "run-skill".to_string(),
                    name: "Run Formatter".to_string(),
                    step_type: StepType::Skill,
                    depends_on: vec!["gather-news".to_string()],
                    condition: None,
                    timeout_secs: None,
                    retry: None,
                    config: StepConfig::Skill {
                        skill: "markdown-formatter".to_string(),
                        input: Some("{{ steps.gather-news.output }}".to_string()),
                    },
                    ui: None,
                },
                StepDefinition {
                    id: "transform".to_string(),
                    name: "Transform Data".to_string(),
                    step_type: StepType::Code,
                    depends_on: vec!["run-skill".to_string()],
                    condition: None,
                    timeout_secs: None,
                    retry: None,
                    config: StepConfig::Code {
                        language: CodeLanguage::TypeScript,
                        source: "export default (ctx) => ctx.steps['run-skill'].output.toUpperCase()".to_string(),
                    },
                    ui: None,
                },
                StepDefinition {
                    id: "notify".to_string(),
                    name: "Send Notification".to_string(),
                    step_type: StepType::Http,
                    depends_on: vec!["transform".to_string()],
                    condition: None,
                    timeout_secs: Some(30),
                    retry: Some(RetryConfig {
                        max_attempts: 3,
                        strategy: RetryStrategy::Simple,
                    }),
                    config: StepConfig::Http {
                        method: "POST".to_string(),
                        url: "https://hooks.slack.com/services/xxx".to_string(),
                        headers: Some(HashMap::from([(
                            "Content-Type".to_string(),
                            "application/json".to_string(),
                        )])),
                        body: Some(r#"{"text":"{{ steps.transform.output }}"}"#.to_string()),
                    },
                    ui: None,
                },
                StepDefinition {
                    id: "check-quality".to_string(),
                    name: "Check Quality".to_string(),
                    step_type: StepType::Conditional,
                    depends_on: vec!["gather-news".to_string()],
                    condition: None,
                    timeout_secs: None,
                    retry: None,
                    config: StepConfig::Conditional {
                        condition: "steps['gather-news'].output|length > 0".to_string(),
                        then_steps: vec!["run-skill".to_string()],
                        else_steps: vec!["notify".to_string()],
                    },
                    ui: None,
                },
                StepDefinition {
                    id: "retry-loop".to_string(),
                    name: "Retry Loop".to_string(),
                    step_type: StepType::Loop,
                    depends_on: vec![],
                    condition: None,
                    timeout_secs: None,
                    retry: None,
                    config: StepConfig::Loop {
                        condition: "context.attempts < 3".to_string(),
                        max_iterations: Some(5),
                        body_steps: vec!["gather-news".to_string()],
                    },
                    ui: None,
                },
                StepDefinition {
                    id: "human-review".to_string(),
                    name: "Human Review".to_string(),
                    step_type: StepType::Approval,
                    depends_on: vec!["transform".to_string()],
                    condition: None,
                    timeout_secs: None,
                    retry: None,
                    config: StepConfig::Approval {
                        prompt: "Review the generated digest before publishing".to_string(),
                        timeout_secs: Some(3600),
                    },
                    ui: None,
                },
                StepDefinition {
                    id: "run-sub".to_string(),
                    name: "Run Sub-Workflow".to_string(),
                    step_type: StepType::SubWorkflow,
                    depends_on: vec!["human-review".to_string()],
                    condition: Some("context.approved == true".to_string()),
                    timeout_secs: None,
                    retry: Some(RetryConfig {
                        max_attempts: 2,
                        strategy: RetryStrategy::LlmSelfCorrect,
                    }),
                    config: StepConfig::SubWorkflow {
                        workflow_name: "publish-digest".to_string(),
                        input: Some(json!({"content": "{{ steps.transform.output }}"})),
                    },
                    ui: None,
                },
            ],
            metadata: HashMap::from([("created_by".to_string(), json!("builder"))]),
        }
    }

    // -----------------------------------------------------------------------
    // YAML roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_workflow_definition_yaml_roundtrip() {
        let original = sample_workflow();
        let yaml = serde_yaml_ng::to_string(&original).expect("serialize to YAML");

        // Verify it's valid YAML text
        assert!(yaml.contains("daily-digest"));
        assert!(yaml.contains("gather-news"));
        assert!(yaml.contains("type: agent"));
        assert!(yaml.contains("type: cron"));
        assert!(yaml.contains("type: webhook"));

        // Roundtrip
        let parsed: WorkflowDefinition =
            serde_yaml_ng::from_str(&yaml).expect("deserialize from YAML");
        assert_eq!(parsed.name, "daily-digest");
        assert_eq!(parsed.version, "1.0.0");
        assert_eq!(parsed.concurrency, Some(1));
        assert_eq!(parsed.triggers.len(), 5);
        assert_eq!(parsed.steps.len(), 8);
    }

    #[test]
    fn test_workflow_definition_json_roundtrip() {
        let original = sample_workflow();
        let json_str = serde_json::to_string_pretty(&original).expect("serialize to JSON");
        let parsed: WorkflowDefinition =
            serde_json::from_str(&json_str).expect("deserialize from JSON");
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.steps.len(), original.steps.len());
        assert_eq!(parsed.triggers.len(), original.triggers.len());
    }

    // -----------------------------------------------------------------------
    // StepConfig all variants
    // -----------------------------------------------------------------------

    #[test]
    fn test_step_config_agent_serde() {
        let config = StepConfig::Agent {
            bot: "researcher".to_string(),
            prompt: "Find news".to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"agent\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Agent { .. }));
    }

    #[test]
    fn test_step_config_skill_serde() {
        let config = StepConfig::Skill {
            skill: "formatter".to_string(),
            input: Some("hello".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"skill\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Skill { .. }));
    }

    #[test]
    fn test_step_config_code_serde() {
        let config = StepConfig::Code {
            language: CodeLanguage::TypeScript,
            source: "console.log('hi')".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"code\""));
        assert!(json.contains("\"language\":\"type_script\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Code { .. }));
    }

    #[test]
    fn test_step_config_http_serde() {
        let config = StepConfig::Http {
            method: "POST".to_string(),
            url: "https://example.com".to_string(),
            headers: Some(HashMap::from([(
                "Authorization".to_string(),
                "Bearer xxx".to_string(),
            )])),
            body: Some(r#"{"key":"value"}"#.to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"http\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Http { .. }));
    }

    #[test]
    fn test_step_config_conditional_serde() {
        let config = StepConfig::Conditional {
            condition: "x > 5".to_string(),
            then_steps: vec!["a".to_string()],
            else_steps: vec!["b".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"conditional\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Conditional { .. }));
    }

    #[test]
    fn test_step_config_loop_serde() {
        let config = StepConfig::Loop {
            condition: "count < 10".to_string(),
            max_iterations: Some(10),
            body_steps: vec!["step-a".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"loop\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Loop { .. }));
    }

    #[test]
    fn test_step_config_approval_serde() {
        let config = StepConfig::Approval {
            prompt: "Please review".to_string(),
            timeout_secs: Some(3600),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"approval\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::Approval { .. }));
    }

    #[test]
    fn test_step_config_sub_workflow_serde() {
        let config = StepConfig::SubWorkflow {
            workflow_name: "publish".to_string(),
            input: Some(json!({"key": "value"})),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"sub_workflow\""));
        let parsed: StepConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, StepConfig::SubWorkflow { .. }));
    }

    // -----------------------------------------------------------------------
    // TriggerConfig all variants
    // -----------------------------------------------------------------------

    #[test]
    fn test_trigger_config_manual_serde() {
        let trigger = TriggerConfig::Manual {};
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"manual\""));
        let parsed: TriggerConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TriggerConfig::Manual {}));
    }

    #[test]
    fn test_trigger_config_cron_serde() {
        let trigger = TriggerConfig::Cron {
            schedule: "0 9 * * *".to_string(),
            timezone: Some("UTC".to_string()),
        };
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"cron\""));
        let parsed: TriggerConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TriggerConfig::Cron { .. }));
    }

    #[test]
    fn test_trigger_config_webhook_serde() {
        let trigger = TriggerConfig::Webhook {
            path: "/hook".to_string(),
            auth: Some(WebhookAuth::BearerToken {
                secret_name: "MY_TOKEN".to_string(),
            }),
            when: Some("event.type == 'push'".to_string()),
        };
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"webhook\""));
        assert!(json.contains("\"type\":\"bearer_token\""));
        let parsed: TriggerConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TriggerConfig::Webhook { .. }));
    }

    #[test]
    fn test_trigger_config_event_serde() {
        let trigger = TriggerConfig::Event {
            source: "internal".to_string(),
            event_type: "bot_created".to_string(),
            when: None,
        };
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"event\""));
        let parsed: TriggerConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TriggerConfig::Event { .. }));
    }

    #[test]
    fn test_trigger_config_file_watch_serde() {
        let trigger = TriggerConfig::FileWatch {
            paths: vec!["/data".to_string()],
            patterns: Some(vec!["*.json".to_string()]),
            when: None,
        };
        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"file_watch\""));
        let parsed: TriggerConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TriggerConfig::FileWatch { .. }));
    }

    // -----------------------------------------------------------------------
    // WebhookAuth variants
    // -----------------------------------------------------------------------

    #[test]
    fn test_webhook_auth_hmac_serde() {
        let auth = WebhookAuth::HmacSha256 {
            secret_name: "MY_SECRET".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"hmac_sha256\""));
        let parsed: WebhookAuth = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WebhookAuth::HmacSha256 { .. }));
    }

    #[test]
    fn test_webhook_auth_bearer_serde() {
        let auth = WebhookAuth::BearerToken {
            secret_name: "MY_TOKEN".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"bearer_token\""));
        let parsed: WebhookAuth = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WebhookAuth::BearerToken { .. }));
    }

    // -----------------------------------------------------------------------
    // Status enums
    // -----------------------------------------------------------------------

    #[test]
    fn test_workflow_run_status_serde() {
        for status in [
            WorkflowRunStatus::Pending,
            WorkflowRunStatus::Running,
            WorkflowRunStatus::Paused,
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Failed,
            WorkflowRunStatus::Crashed,
            WorkflowRunStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: WorkflowRunStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_workflow_step_status_serde() {
        for status in [
            WorkflowStepStatus::Pending,
            WorkflowStepStatus::Running,
            WorkflowStepStatus::Completed,
            WorkflowStepStatus::Failed,
            WorkflowStepStatus::Skipped,
            WorkflowStepStatus::WaitingApproval,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: WorkflowStepStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    // -----------------------------------------------------------------------
    // WorkflowRun and WorkflowStepLog roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_workflow_run_json_roundtrip() {
        let run = WorkflowRun {
            id: Uuid::now_v7(),
            workflow_id: Uuid::now_v7(),
            workflow_name: "daily-digest".to_string(),
            status: WorkflowRunStatus::Running,
            trigger_type: "cron".to_string(),
            trigger_payload: Some(json!({"schedule": "0 9 * * *"})),
            context: json!({"steps": {}}),
            started_at: Utc::now(),
            completed_at: None,
            error: None,
            concurrency_key: Some("daily-digest".to_string()),
        };
        let json_str = serde_json::to_string(&run).unwrap();
        let parsed: WorkflowRun = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.workflow_name, "daily-digest");
        assert_eq!(parsed.status, WorkflowRunStatus::Running);
    }

    #[test]
    fn test_workflow_step_log_json_roundtrip() {
        let log = WorkflowStepLog {
            id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            step_id: "gather-news".to_string(),
            step_name: "Gather News".to_string(),
            status: WorkflowStepStatus::Completed,
            attempt: 1,
            idempotency_key: Some("run-123-gather-news-1".to_string()),
            input: Some(json!({"query": "AI news"})),
            output: Some(json!({"articles": ["Article 1", "Article 2"]})),
            error: None,
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
        };
        let json_str = serde_json::to_string(&log).unwrap();
        let parsed: WorkflowStepLog = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.step_id, "gather-news");
        assert_eq!(parsed.status, WorkflowStepStatus::Completed);
        assert_eq!(parsed.attempt, 1);
    }

    // -----------------------------------------------------------------------
    // Owner enum
    // -----------------------------------------------------------------------

    #[test]
    fn test_workflow_owner_bot_serde() {
        let owner = WorkflowOwner::Bot {
            bot_id: Uuid::now_v7(),
            slug: "researcher".to_string(),
        };
        let json = serde_json::to_string(&owner).unwrap();
        assert!(json.contains("\"type\":\"bot\""));
        let parsed: WorkflowOwner = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WorkflowOwner::Bot { .. }));
    }

    #[test]
    fn test_workflow_owner_global_serde() {
        let owner = WorkflowOwner::Global;
        let json = serde_json::to_string(&owner).unwrap();
        assert!(json.contains("\"type\":\"global\""));
        let parsed: WorkflowOwner = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WorkflowOwner::Global));
    }

    // -----------------------------------------------------------------------
    // RetryConfig
    // -----------------------------------------------------------------------

    #[test]
    fn test_retry_config_default_max_attempts() {
        let yaml = r#"strategy: simple"#;
        let config: RetryConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.max_attempts, 3); // default
        assert_eq!(config.strategy, RetryStrategy::Simple);
    }

    #[test]
    fn test_retry_config_llm_self_correct() {
        let config = RetryConfig {
            max_attempts: 2,
            strategy: RetryStrategy::LlmSelfCorrect,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"llm_self_correct\""));
        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_attempts, 2);
        assert_eq!(parsed.strategy, RetryStrategy::LlmSelfCorrect);
    }

    // -----------------------------------------------------------------------
    // StepUiMetadata
    // -----------------------------------------------------------------------

    #[test]
    fn test_step_ui_metadata_serde() {
        let meta = StepUiMetadata {
            position: Some(UiPosition { x: 100.0, y: 200.5 }),
            group: Some("analysis-group".to_string()),
            collapsed: Some(true),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("100.0"));
        assert!(json.contains("analysis-group"));
        let parsed: StepUiMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.position.unwrap().x, 100.0);
        assert_eq!(parsed.group.as_deref(), Some("analysis-group"));
    }

    // -----------------------------------------------------------------------
    // YAML from-scratch parse (realistic workflow YAML)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_realistic_yaml_workflow() {
        let yaml = r#"
id: "01938e90-0000-7000-8000-000000000001"
name: daily-digest
description: Gather news and summarize
version: "1.0"
owner:
  type: bot
  bot_id: "01938e90-0000-7000-8000-000000000002"
  slug: researcher
concurrency: 1
triggers:
  - type: cron
    schedule: "0 9 * * *"
  - type: manual
steps:
  - id: gather
    name: Gather News
    type: agent
    config:
      type: agent
      bot: researcher
      prompt: Find the top 5 AI news stories
    timeout_secs: 120
  - id: analyze
    name: Analyze
    type: agent
    depends_on: [gather]
    config:
      type: agent
      bot: analyst
      prompt: Analyze trends
    retry:
      max_attempts: 3
      strategy: llm_self_correct
"#;
        let wf: WorkflowDefinition = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(wf.name, "daily-digest");
        assert_eq!(wf.version, "1.0");
        assert_eq!(wf.concurrency, Some(1));
        assert_eq!(wf.triggers.len(), 2);
        assert_eq!(wf.steps.len(), 2);
        assert_eq!(wf.steps[1].depends_on, vec!["gather"]);
        assert!(wf.steps[1].retry.is_some());
        assert_eq!(
            wf.steps[1].retry.as_ref().unwrap().strategy,
            RetryStrategy::LlmSelfCorrect
        );
    }
}
