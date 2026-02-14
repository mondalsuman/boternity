//! Workflow execution context with step output tracking and template resolution.
//!
//! `WorkflowContext` is the mutable state that flows through a workflow run.
//! It stores step outputs, trigger payloads, and user-defined variables, with
//! size limits to prevent unbounded memory growth.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::definition::WorkflowError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum size of a single step output (1 MB).
pub const MAX_STEP_OUTPUT_SIZE: usize = 1_048_576;

/// Maximum total size of all context data (10 MB).
pub const MAX_CONTEXT_SIZE: usize = 10_485_760;

// ---------------------------------------------------------------------------
// WorkflowContext
// ---------------------------------------------------------------------------

/// Mutable execution context that tracks state across a workflow run.
///
/// Stores step outputs, trigger payloads, and user-defined variables.
/// Supports template resolution for `{{ steps.<id>.output }}` patterns
/// and JSON serialization for checkpointing/resumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    /// Step outputs keyed by step ID.
    pub step_outputs: HashMap<String, Value>,
    /// User-defined variables.
    pub variables: HashMap<String, Value>,
    /// Trigger payload (webhook body, cron metadata, etc.).
    pub trigger_payload: Option<Value>,
    /// Workflow name.
    pub workflow_name: String,
    /// Run ID.
    pub run_id: Uuid,
}

impl WorkflowContext {
    /// Create a new workflow context for a run.
    pub fn new(
        workflow_name: String,
        run_id: Uuid,
        trigger_payload: Option<Value>,
    ) -> Self {
        Self {
            step_outputs: HashMap::new(),
            variables: HashMap::new(),
            trigger_payload,
            workflow_name,
            run_id,
        }
    }

    /// Store the output of a completed step.
    ///
    /// Enforces `MAX_STEP_OUTPUT_SIZE` (1 MB) per output. If the output
    /// exceeds this limit, it is truncated to a JSON string indicating
    /// the overflow. Also enforces `MAX_CONTEXT_SIZE` (10 MB) total.
    pub fn set_step_output(
        &mut self,
        step_id: &str,
        output: Value,
    ) -> Result<(), WorkflowError> {
        let serialized = serde_json::to_string(&output)
            .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;

        if serialized.len() > MAX_STEP_OUTPUT_SIZE {
            tracing::warn!(
                step_id,
                size = serialized.len(),
                max = MAX_STEP_OUTPUT_SIZE,
                "step output exceeds size limit, truncating"
            );
            let truncated = json!({
                "_truncated": true,
                "_original_size": serialized.len(),
                "_message": format!(
                    "output exceeded {} byte limit and was truncated",
                    MAX_STEP_OUTPUT_SIZE
                )
            });
            self.step_outputs.insert(step_id.to_string(), truncated);
        } else {
            self.step_outputs.insert(step_id.to_string(), output);
        }

        // Check total context size
        let total = self.total_size();
        if total > MAX_CONTEXT_SIZE {
            return Err(WorkflowError::ExecutionError(format!(
                "total context size ({} bytes) exceeds maximum ({} bytes)",
                total, MAX_CONTEXT_SIZE
            )));
        }

        Ok(())
    }

    /// Get the output of a completed step.
    pub fn get_step_output(&self, step_id: &str) -> Option<&Value> {
        self.step_outputs.get(step_id)
    }

    /// Resolve template variables in a string.
    ///
    /// Supports patterns:
    /// - `{{ steps.<step_id>.output }}` -- replaced with step output as string
    /// - `{{ trigger.<field> }}` -- replaced with trigger payload field
    /// - `{{ variables.<name> }}` -- replaced with variable value
    ///
    /// Unknown references are left as-is (not an error).
    pub fn resolve_template(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Resolve {{ steps.<id>.output }}
        while let Some(start) = result.find("{{ steps.") {
            if let Some(end) = result[start..].find(" }}") {
                let end = start + end + 3; // include " }}"
                let expr = &result[start + 3..end - 3].trim(); // strip "{{ " and " }}"

                // Parse "steps.<id>.output"
                if let Some(rest) = expr.strip_prefix("steps.") {
                    if let Some(dot_pos) = rest.find('.') {
                        let step_id = &rest[..dot_pos];
                        let field = &rest[dot_pos + 1..];
                        if field == "output" {
                            if let Some(output) = self.step_outputs.get(step_id) {
                                let replacement = value_to_string(output);
                                result.replace_range(start..end, &replacement);
                                continue;
                            }
                        }
                    }
                }

                // If we couldn't resolve, skip past this marker
                // to avoid infinite loops
                if start + 3 < result.len() {
                    // Move past the opening {{ to avoid re-matching
                    let remaining = result[start + 3..].to_string();
                    result = format!("{}{{{{ {}", &result[..start], remaining);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Resolve {{ trigger.<field> }}
        while let Some(start) = result.find("{{ trigger.") {
            if let Some(end) = result[start..].find(" }}") {
                let end = start + end + 3;
                let expr = &result[start + 3..end - 3].trim();

                if let Some(field) = expr.strip_prefix("trigger.") {
                    if let Some(payload) = &self.trigger_payload {
                        if let Some(val) = payload.get(field) {
                            let replacement = value_to_string(val);
                            result.replace_range(start..end, &replacement);
                            continue;
                        }
                    }
                }

                // Skip unresolvable
                if start + 3 < result.len() {
                    let remaining = result[start + 3..].to_string();
                    result = format!("{}{{{{ {}", &result[..start], remaining);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Resolve {{ variables.<name> }}
        while let Some(start) = result.find("{{ variables.") {
            if let Some(end) = result[start..].find(" }}") {
                let end = start + end + 3;
                let expr = &result[start + 3..end - 3].trim();

                if let Some(name) = expr.strip_prefix("variables.") {
                    if let Some(val) = self.variables.get(name) {
                        let replacement = value_to_string(val);
                        result.replace_range(start..end, &replacement);
                        continue;
                    }
                }

                // Skip unresolvable
                if start + 3 < result.len() {
                    let remaining = result[start + 3..].to_string();
                    result = format!("{}{{{{ {}", &result[..start], remaining);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        result
    }

    /// Compute the total serialized size of all context data in bytes.
    pub fn total_size(&self) -> usize {
        let outputs_size: usize = self
            .step_outputs
            .values()
            .map(|v| serde_json::to_string(v).map(|s| s.len()).unwrap_or(0))
            .sum();
        let variables_size: usize = self
            .variables
            .values()
            .map(|v| serde_json::to_string(v).map(|s| s.len()).unwrap_or(0))
            .sum();
        let trigger_size = self
            .trigger_payload
            .as_ref()
            .map(|v| serde_json::to_string(v).map(|s| s.len()).unwrap_or(0))
            .unwrap_or(0);
        outputs_size + variables_size + trigger_size
    }

    /// Serialize the entire context to JSON for checkpointing.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(json!({}))
    }

    /// Restore a context from a JSON checkpoint.
    pub fn from_json(value: Value) -> Result<Self, WorkflowError> {
        serde_json::from_value(value)
            .map_err(|e| WorkflowError::ParseError(format!("failed to restore context: {}", e)))
    }

    /// Build a JSON object suitable for JEXL expression evaluation.
    ///
    /// Shape:
    /// ```json
    /// {
    ///   "steps": { "<step_id>": { "output": <value> }, ... },
    ///   "trigger": <trigger_payload or {}>,
    ///   "variables": { ... },
    ///   "workflow": { "name": "...", "run_id": "..." }
    /// }
    /// ```
    pub fn to_expression_context(&self) -> Value {
        let mut steps = serde_json::Map::new();
        for (id, output) in &self.step_outputs {
            steps.insert(id.clone(), json!({ "output": output }));
        }

        json!({
            "steps": steps,
            "trigger": self.trigger_payload.clone().unwrap_or(json!({})),
            "variables": self.variables,
            "workflow": {
                "name": self.workflow_name,
                "run_id": self.run_id.to_string(),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a JSON value to a display string for template resolution.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        // For objects/arrays, return compact JSON
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_context() -> WorkflowContext {
        WorkflowContext::new(
            "test-workflow".to_string(),
            Uuid::now_v7(),
            Some(json!({ "source": "github", "event": "push" })),
        )
    }

    // -----------------------------------------------------------------------
    // Basic operations
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_context() {
        let ctx = test_context();
        assert_eq!(ctx.workflow_name, "test-workflow");
        assert!(ctx.step_outputs.is_empty());
        assert!(ctx.variables.is_empty());
        assert!(ctx.trigger_payload.is_some());
    }

    #[test]
    fn test_set_and_get_step_output() {
        let mut ctx = test_context();
        ctx.set_step_output("gather", json!("news articles"))
            .unwrap();

        assert_eq!(
            ctx.get_step_output("gather"),
            Some(&json!("news articles"))
        );
        assert_eq!(ctx.get_step_output("missing"), None);
    }

    // -----------------------------------------------------------------------
    // Template resolution
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_step_output_template() {
        let mut ctx = test_context();
        ctx.set_step_output("gather", json!("top 5 AI news"))
            .unwrap();

        let result = ctx.resolve_template("Results: {{ steps.gather.output }}");
        assert_eq!(result, "Results: top 5 AI news");
    }

    #[test]
    fn test_resolve_trigger_template() {
        let ctx = test_context();
        let result = ctx.resolve_template("Source: {{ trigger.source }}");
        assert_eq!(result, "Source: github");
    }

    #[test]
    fn test_resolve_variable_template() {
        let mut ctx = test_context();
        ctx.variables
            .insert("max_retries".to_string(), json!(3));

        let result = ctx.resolve_template("Retries: {{ variables.max_retries }}");
        assert_eq!(result, "Retries: 3");
    }

    #[test]
    fn test_resolve_unknown_reference_left_asis() {
        let ctx = test_context();
        // Unknown step references should not cause errors
        let template = "Hello world";
        let result = ctx.resolve_template(template);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_resolve_multiple_templates() {
        let mut ctx = test_context();
        ctx.set_step_output("gather", json!("news")).unwrap();
        ctx.set_step_output("analyze", json!("trends")).unwrap();

        let result = ctx.resolve_template(
            "{{ steps.gather.output }} and {{ steps.analyze.output }}",
        );
        assert_eq!(result, "news and trends");
    }

    // -----------------------------------------------------------------------
    // Size limits
    // -----------------------------------------------------------------------

    #[test]
    fn test_step_output_size_limit_truncates() {
        let mut ctx = test_context();
        // Create a string larger than 1 MB
        let large_string = "x".repeat(MAX_STEP_OUTPUT_SIZE + 100);
        ctx.set_step_output("big", json!(large_string)).unwrap();

        let output = ctx.get_step_output("big").unwrap();
        assert!(output.get("_truncated").is_some());
        assert_eq!(output["_truncated"], json!(true));
    }

    #[test]
    fn test_total_context_size_check() {
        let ctx = test_context();
        // Empty context should have small size
        assert!(ctx.total_size() < 1000);
    }

    // -----------------------------------------------------------------------
    // JSON checkpoint roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_checkpoint_roundtrip() {
        let mut ctx = test_context();
        ctx.set_step_output("gather", json!("news")).unwrap();
        ctx.variables
            .insert("count".to_string(), json!(42));

        let json = ctx.to_json();
        let restored = WorkflowContext::from_json(json).unwrap();

        assert_eq!(restored.workflow_name, "test-workflow");
        assert_eq!(
            restored.get_step_output("gather"),
            Some(&json!("news"))
        );
        assert_eq!(
            restored.variables.get("count"),
            Some(&json!(42))
        );
    }

    // -----------------------------------------------------------------------
    // Expression context
    // -----------------------------------------------------------------------

    #[test]
    fn test_to_expression_context() {
        let mut ctx = test_context();
        ctx.set_step_output("gather", json!("news")).unwrap();

        let expr_ctx = ctx.to_expression_context();
        assert_eq!(expr_ctx["steps"]["gather"]["output"], json!("news"));
        assert_eq!(expr_ctx["trigger"]["source"], json!("github"));
        assert_eq!(expr_ctx["workflow"]["name"], json!("test-workflow"));
    }
}
