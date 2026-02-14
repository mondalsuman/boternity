//! Workflow definition parsing, validation, and filesystem operations.
//!
//! Converts between YAML files and the canonical `WorkflowDefinition` IR,
//! validates structural constraints (unique IDs, valid dependencies, name format),
//! and provides discovery for workflow files on disk.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use boternity_types::workflow::{StepConfig, WorkflowDefinition};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during workflow operations.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// YAML/JSON parse failure.
    #[error("parse error: {0}")]
    ParseError(String),

    /// Structural validation failure.
    #[error("validation error: {0}")]
    ValidationError(String),

    /// Filesystem I/O failure.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Dependency graph contains a cycle.
    #[error("cycle detected: {0}")]
    CycleDetected(String),

    /// A step references an unknown dependency.
    #[error("unknown dependency: {0}")]
    UnknownDependency(String),

    /// JEXL or template expression error.
    #[error("expression error: {0}")]
    ExpressionError(String),

    /// Runtime execution failure.
    #[error("execution error: {0}")]
    ExecutionError(String),

    /// Step or workflow exceeded its timeout.
    #[error("timeout exceeded")]
    TimeoutError,

    /// Concurrency limit for this workflow was reached.
    #[error("concurrency limit reached")]
    ConcurrencyLimitReached,

    /// Sub-workflow nesting depth exceeded.
    #[error("sub-workflow depth {depth} exceeds maximum {max}")]
    SubWorkflowDepthExceeded { depth: u32, max: u32 },
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a YAML string into a validated `WorkflowDefinition`.
///
/// Runs `validate_definition` after deserialization, so the returned value
/// is guaranteed to be structurally valid.
pub fn parse_workflow_yaml(yaml: &str) -> Result<WorkflowDefinition, WorkflowError> {
    let def: WorkflowDefinition =
        serde_yaml_ng::from_str(yaml).map_err(|e| WorkflowError::ParseError(e.to_string()))?;
    validate_definition(&def)?;
    Ok(def)
}

/// Serialize a `WorkflowDefinition` to a YAML string.
pub fn serialize_workflow_yaml(def: &WorkflowDefinition) -> Result<String, WorkflowError> {
    serde_yaml_ng::to_string(def).map_err(|e| WorkflowError::ParseError(e.to_string()))
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate structural constraints on a `WorkflowDefinition`.
///
/// Checks:
/// - Name is non-empty and contains only alphanumeric characters and hyphens
/// - At least one step exists
/// - All step IDs are unique
/// - All `depends_on` references point to existing step IDs
/// - Conditional/Loop body step references point to existing step IDs
/// - Concurrency >= 1 if set
/// - Timeout > 0 if set
pub fn validate_definition(def: &WorkflowDefinition) -> Result<(), WorkflowError> {
    // Name format: non-empty, alphanumeric + hyphens only
    if def.name.is_empty() {
        return Err(WorkflowError::ValidationError(
            "workflow name must not be empty".to_string(),
        ));
    }
    if !def
        .name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-')
    {
        return Err(WorkflowError::ValidationError(format!(
            "workflow name '{}' contains invalid characters (only alphanumeric and hyphens allowed)",
            def.name
        )));
    }

    // At least one step
    if def.steps.is_empty() {
        return Err(WorkflowError::ValidationError(
            "workflow must have at least one step".to_string(),
        ));
    }

    // Unique step IDs
    let mut seen_ids = HashSet::new();
    for step in &def.steps {
        if !seen_ids.insert(step.id.as_str()) {
            return Err(WorkflowError::ValidationError(format!(
                "duplicate step ID: '{}'",
                step.id
            )));
        }
    }

    // depends_on references must be valid
    for step in &def.steps {
        for dep in &step.depends_on {
            if !seen_ids.contains(dep.as_str()) {
                return Err(WorkflowError::UnknownDependency(format!(
                    "step '{}' depends on unknown step '{}'",
                    step.id, dep
                )));
            }
        }
    }

    // Conditional/Loop body step references must be valid
    for step in &def.steps {
        match &step.config {
            StepConfig::Conditional {
                then_steps,
                else_steps,
                ..
            } => {
                for ref_id in then_steps.iter().chain(else_steps.iter()) {
                    if !seen_ids.contains(ref_id.as_str()) {
                        return Err(WorkflowError::ValidationError(format!(
                            "conditional step '{}' references unknown step '{}'",
                            step.id, ref_id
                        )));
                    }
                }
            }
            StepConfig::Loop { body_steps, .. } => {
                for ref_id in body_steps {
                    if !seen_ids.contains(ref_id.as_str()) {
                        return Err(WorkflowError::ValidationError(format!(
                            "loop step '{}' references unknown step '{}'",
                            step.id, ref_id
                        )));
                    }
                }
            }
            _ => {}
        }
    }

    // Concurrency >= 1 if set
    if let Some(c) = def.concurrency {
        if c < 1 {
            return Err(WorkflowError::ValidationError(
                "concurrency must be >= 1".to_string(),
            ));
        }
    }

    // Timeout > 0 if set
    if let Some(t) = def.timeout_secs {
        if t == 0 {
            return Err(WorkflowError::ValidationError(
                "timeout must be > 0".to_string(),
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Filesystem operations
// ---------------------------------------------------------------------------

/// Load a workflow definition from a YAML file.
pub fn load_workflow_file(path: &Path) -> Result<WorkflowDefinition, WorkflowError> {
    let content = std::fs::read_to_string(path)?;
    parse_workflow_yaml(&content)
}

/// Save a workflow definition to a YAML file.
///
/// Creates parent directories if they don't exist.
pub fn save_workflow_file(path: &Path, def: &WorkflowDefinition) -> Result<(), WorkflowError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = serialize_workflow_yaml(def)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

/// Discover all workflow YAML files under `base_dir`.
///
/// Scans for `.yaml` and `.yml` files recursively. Each file is parsed and
/// returned alongside its path. Files that fail to parse are silently skipped
/// (logged in production, but not returned as errors).
pub fn discover_workflows(
    base_dir: &Path,
) -> Result<Vec<(PathBuf, WorkflowDefinition)>, WorkflowError> {
    let mut results = Vec::new();
    if !base_dir.exists() {
        return Ok(results);
    }
    discover_recursive(base_dir, &mut results)?;
    Ok(results)
}

fn discover_recursive(
    dir: &Path,
    results: &mut Vec<(PathBuf, WorkflowDefinition)>,
) -> Result<(), WorkflowError> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            discover_recursive(&path, results)?;
        } else if let Some(ext) = path.extension() {
            if ext == "yaml" || ext == "yml" {
                match load_workflow_file(&path) {
                    Ok(def) => results.push((path, def)),
                    Err(_) => {
                        // Skip files that fail to parse (they may not be workflows)
                        tracing::warn!(?path, "skipping unparseable workflow file");
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::workflow::{
        StepConfig, StepDefinition, StepType, TriggerConfig, WorkflowDefinition, WorkflowOwner,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Helper: build a minimal valid workflow definition.
    fn minimal_workflow(name: &str, steps: Vec<StepDefinition>) -> WorkflowDefinition {
        WorkflowDefinition {
            id: Uuid::now_v7(),
            name: name.to_string(),
            description: None,
            version: "1.0.0".to_string(),
            owner: WorkflowOwner::Global,
            concurrency: None,
            timeout_secs: None,
            triggers: vec![TriggerConfig::Manual {}],
            steps,
            metadata: HashMap::new(),
        }
    }

    /// Helper: build a simple agent step.
    fn agent_step(id: &str, depends_on: Vec<&str>) -> StepDefinition {
        StepDefinition {
            id: id.to_string(),
            name: id.to_string(),
            step_type: StepType::Agent,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            condition: None,
            timeout_secs: None,
            retry: None,
            config: StepConfig::Agent {
                bot: "test-bot".to_string(),
                prompt: "do something".to_string(),
                model: None,
            },
            ui: None,
        }
    }

    // -----------------------------------------------------------------------
    // YAML roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_yaml_roundtrip() {
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
"#;
        let def = parse_workflow_yaml(yaml).expect("should parse");
        assert_eq!(def.name, "daily-digest");
        assert_eq!(def.steps.len(), 2);
        assert_eq!(def.triggers.len(), 2);
        assert_eq!(def.concurrency, Some(1));

        // Serialize back to YAML and re-parse
        let yaml2 = serialize_workflow_yaml(&def).expect("should serialize");
        let def2 = parse_workflow_yaml(&yaml2).expect("should re-parse");
        assert_eq!(def2.name, def.name);
        assert_eq!(def2.steps.len(), def.steps.len());
        assert_eq!(def2.triggers.len(), def.triggers.len());
    }

    // -----------------------------------------------------------------------
    // Validation: duplicate step IDs
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_rejects_duplicate_step_ids() {
        let def = minimal_workflow(
            "test-wf",
            vec![agent_step("step-a", vec![]), agent_step("step-a", vec![])],
        );
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate step ID"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // Validation: unknown depends_on
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_rejects_unknown_dependency() {
        let def = minimal_workflow(
            "test-wf",
            vec![agent_step("step-a", vec!["nonexistent"])],
        );
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown step"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // Validation: empty workflow
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_rejects_empty_workflow() {
        let def = minimal_workflow("test-wf", vec![]);
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("at least one step"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // Validation: invalid name
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_rejects_invalid_name() {
        let def = minimal_workflow("has spaces!", vec![agent_step("a", vec![])]);
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid characters"), "got: {msg}");
    }

    #[test]
    fn test_validation_rejects_empty_name() {
        let def = minimal_workflow("", vec![agent_step("a", vec![])]);
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("must not be empty"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // Validation: concurrency and timeout
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_rejects_zero_timeout() {
        let mut def = minimal_workflow("test-wf", vec![agent_step("a", vec![])]);
        def.timeout_secs = Some(0);
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("timeout must be > 0"), "got: {msg}");
    }

    // -----------------------------------------------------------------------
    // Validation: orphan conditional/loop references
    // -----------------------------------------------------------------------

    #[test]
    fn test_validation_rejects_orphan_conditional_reference() {
        let mut def = minimal_workflow("test-wf", vec![agent_step("check", vec![])]);
        def.steps[0].config = StepConfig::Conditional {
            condition: "true".to_string(),
            then_steps: vec!["nonexistent".to_string()],
            else_steps: vec![],
        };
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("references unknown step"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_validation_rejects_orphan_loop_reference() {
        let mut def = minimal_workflow("test-wf", vec![agent_step("loop-step", vec![])]);
        def.steps[0].config = StepConfig::Loop {
            condition: "true".to_string(),
            max_iterations: Some(5),
            body_steps: vec!["missing".to_string()],
        };
        let err = validate_definition(&def).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("references unknown step"),
            "got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Filesystem: save and load roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_and_load_workflow_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows/test.yaml");

        let def = minimal_workflow("test-wf", vec![agent_step("a", vec![])]);
        save_workflow_file(&path, &def).expect("should save");

        let loaded = load_workflow_file(&path).expect("should load");
        assert_eq!(loaded.name, "test-wf");
        assert_eq!(loaded.steps.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Filesystem: discover workflows
    // -----------------------------------------------------------------------

    #[test]
    fn test_discover_workflows() {
        let dir = tempfile::tempdir().unwrap();

        // Create two valid workflow files and one non-workflow YAML
        let wf1 = minimal_workflow("wf-one", vec![agent_step("a", vec![])]);
        let wf2 = minimal_workflow("wf-two", vec![agent_step("b", vec![])]);

        save_workflow_file(&dir.path().join("wf1.yaml"), &wf1).unwrap();
        save_workflow_file(&dir.path().join("sub/wf2.yml"), &wf2).unwrap();
        std::fs::write(dir.path().join("not-a-workflow.yaml"), "key: value").unwrap();

        let found = discover_workflows(dir.path()).expect("should discover");
        assert_eq!(found.len(), 2, "should find exactly 2 valid workflows");
    }

    #[test]
    fn test_discover_nonexistent_dir() {
        let result = discover_workflows(Path::new("/nonexistent/path"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
