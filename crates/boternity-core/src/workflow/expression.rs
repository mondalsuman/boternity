//! JEXL expression evaluator for workflow `when` clauses and conditional steps.
//!
//! Wraps `jexl_eval::Evaluator` with pre-registered standard transforms and
//! provides convenience methods for boolean evaluation in workflow contexts.
//!
//! **Security note:** Payloads are always passed as context objects, NEVER
//! interpolated into expression strings.

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during expression evaluation.
#[derive(Debug, thiserror::Error)]
pub enum ExpressionError {
    #[error("Expression evaluation failed: {0}")]
    EvalFailed(String),

    #[error("Expression did not evaluate to a boolean: got {result}")]
    NotBoolean { result: Value },

    #[error("Invalid context: {0}")]
    InvalidContext(String),
}

// ---------------------------------------------------------------------------
// WorkflowEvaluator
// ---------------------------------------------------------------------------

/// JEXL expression evaluator with standard transforms pre-registered.
///
/// Used for:
/// - Trigger `when` clause filtering (e.g. `event.source == 'github'`)
/// - Step `condition` evaluation (e.g. `steps['gather'].output|length > 0`)
/// - Conditional step branching
pub struct WorkflowEvaluator {
    evaluator: jexl_eval::Evaluator<'static>,
}

impl WorkflowEvaluator {
    /// Create a new evaluator with all standard transforms registered.
    pub fn new() -> Self {
        let evaluator = jexl_eval::Evaluator::new()
            // String transforms
            .with_transform("lower", |args: &[Value]| {
                let s = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(json!(s.to_lowercase()))
            })
            .with_transform("upper", |args: &[Value]| {
                let s = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(json!(s.to_uppercase()))
            })
            .with_transform("trim", |args: &[Value]| {
                let s = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(json!(s.trim()))
            })
            .with_transform("split", |args: &[Value]| {
                let s = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let delimiter = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or(",");
                let parts: Vec<&str> = s.split(delimiter).collect();
                Ok(json!(parts))
            })
            // Boolean transforms
            .with_transform("not", |args: &[Value]| {
                let val = args.first().cloned().unwrap_or(Value::Null);
                let truthy = match &val {
                    Value::Bool(b) => *b,
                    Value::Null => false,
                    Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                    Value::String(s) => !s.is_empty(),
                    Value::Array(_) | Value::Object(_) => true,
                };
                Ok(json!(!truthy))
            })
            // String search transforms
            .with_transform("contains", |args: &[Value]| {
                let subject = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let search = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!(subject.contains(search)))
            })
            .with_transform("startsWith", |args: &[Value]| {
                let subject = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let prefix = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!(subject.starts_with(prefix)))
            })
            .with_transform("endsWith", |args: &[Value]| {
                let subject = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let suffix = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!(subject.ends_with(suffix)))
            })
            .with_transform("match", |args: &[Value]| {
                let subject = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let pattern = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                // Simple substring match (not regex, for security/simplicity)
                Ok(json!(subject.contains(pattern)))
            })
            // Length transform (works on strings, arrays, and objects)
            .with_transform("length", |args: &[Value]| {
                let val = args.first().cloned().unwrap_or(Value::Null);
                let len = match &val {
                    Value::String(s) => s.len(),
                    Value::Array(a) => a.len(),
                    Value::Object(o) => o.len(),
                    Value::Null => 0,
                    _ => 0,
                };
                Ok(json!(len as f64))
            });

        Self { evaluator }
    }

    /// Evaluate an expression to a boolean result.
    ///
    /// The `context` must be a JSON object. Expression results are coerced
    /// to boolean using JavaScript-like truthiness rules.
    pub fn evaluate_bool(
        &self,
        expression: &str,
        context: &Value,
    ) -> Result<bool, ExpressionError> {
        if !context.is_object() {
            return Err(ExpressionError::InvalidContext(
                "context must be a JSON object".to_string(),
            ));
        }

        let result = self
            .evaluator
            .eval_in_context(expression, context)
            .map_err(|e| ExpressionError::EvalFailed(e.to_string()))?;

        Ok(Self::value_to_bool(&result))
    }

    /// Evaluate an expression against a workflow context.
    ///
    /// Builds a context JSON object with `steps`, `trigger`, and `variables`
    /// keys from the given `WorkflowContext`.
    pub fn evaluate_in_workflow_context(
        &self,
        expression: &str,
        workflow_context: &WorkflowContext,
    ) -> Result<bool, ExpressionError> {
        let context = workflow_context.to_expression_context();
        self.evaluate_bool(expression, &context)
    }

    /// Evaluate an expression and return the raw JSON value.
    pub fn evaluate_value(
        &self,
        expression: &str,
        context: &Value,
    ) -> Result<Value, ExpressionError> {
        if !context.is_object() {
            return Err(ExpressionError::InvalidContext(
                "context must be a JSON object".to_string(),
            ));
        }

        self.evaluator
            .eval_in_context(expression, context)
            .map_err(|e| ExpressionError::EvalFailed(e.to_string()))
    }

    /// Coerce a JSON value to boolean using JavaScript-like truthiness.
    fn value_to_bool(value: &Value) -> bool {
        match value {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        }
    }
}

impl Default for WorkflowEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WorkflowContext (minimal definition for expression evaluation)
// ---------------------------------------------------------------------------

/// Minimal workflow execution context for expression evaluation.
///
/// This provides the data surface that JEXL expressions can reference:
/// `steps.<id>.output`, `trigger.<field>`, `variables.<name>`.
///
/// The full `WorkflowContext` (with size limits, checkpointing, etc.) is
/// defined in the `context` module; this struct captures just enough for
/// expression evaluation and retry handling.
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Step outputs keyed by step ID.
    pub step_outputs: std::collections::HashMap<String, Value>,
    /// Trigger payload (webhook body, cron metadata, etc.).
    pub trigger_payload: Option<Value>,
    /// User-defined variables.
    pub variables: std::collections::HashMap<String, Value>,
    /// Workflow name.
    pub workflow_name: String,
    /// Run ID (as string for expression context).
    pub run_id: String,
}

impl WorkflowContext {
    /// Build the JSON context object that JEXL expressions evaluate against.
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
                "run_id": self.run_id,
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evaluator() -> WorkflowEvaluator {
        WorkflowEvaluator::new()
    }

    // -------------------------------------------------------------------
    // Basic dot-notation property access
    // -------------------------------------------------------------------

    #[test]
    fn test_dot_notation_nested() {
        let ctx = json!({
            "event": {
                "payload": {
                    "user": {
                        "name": "Alice"
                    }
                }
            }
        });
        let eval = evaluator();
        let result = eval.evaluate_value("event.payload.user.name", &ctx).unwrap();
        assert_eq!(result, json!("Alice"));
    }

    #[test]
    fn test_dot_notation_deep_bool() {
        let ctx = json!({
            "event": {
                "payload": {
                    "user": {
                        "name": "Alice"
                    }
                }
            }
        });
        let eval = evaluator();
        let result = eval
            .evaluate_bool("event.payload.user.name == 'Alice'", &ctx)
            .unwrap();
        assert!(result);
    }

    // -------------------------------------------------------------------
    // Array indexing
    // -------------------------------------------------------------------

    #[test]
    fn test_array_indexing() {
        let ctx = json!({
            "event": {
                "tags": ["rust", "wasm", "jexl"]
            }
        });
        let eval = evaluator();
        let result = eval.evaluate_value("event.tags[0]", &ctx).unwrap();
        assert_eq!(result, json!("rust"));

        let result = eval.evaluate_value("event.tags[2]", &ctx).unwrap();
        assert_eq!(result, json!("jexl"));
    }

    // -------------------------------------------------------------------
    // Boolean operators
    // -------------------------------------------------------------------

    #[test]
    fn test_boolean_and() {
        let ctx = json!({
            "event": {
                "type": "push",
                "branch": "main"
            }
        });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.type == 'push' && event.branch == 'main'", &ctx)
            .unwrap());
        assert!(!eval
            .evaluate_bool("event.type == 'push' && event.branch == 'dev'", &ctx)
            .unwrap());
    }

    #[test]
    fn test_boolean_or() {
        let ctx = json!({
            "event": {
                "type": "push",
                "branch": "dev"
            }
        });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.branch == 'main' || event.branch == 'dev'", &ctx)
            .unwrap());
    }

    // -------------------------------------------------------------------
    // Transforms
    // -------------------------------------------------------------------

    #[test]
    fn test_transform_lower() {
        let ctx = json!({ "event": { "name": "HELLO World" } });
        let eval = evaluator();
        let result = eval.evaluate_value("event.name|lower", &ctx).unwrap();
        assert_eq!(result, json!("hello world"));
    }

    #[test]
    fn test_transform_upper() {
        let ctx = json!({ "event": { "name": "hello" } });
        let eval = evaluator();
        let result = eval.evaluate_value("event.name|upper", &ctx).unwrap();
        assert_eq!(result, json!("HELLO"));
    }

    #[test]
    fn test_transform_trim() {
        let ctx = json!({ "event": { "name": "  hello  " } });
        let eval = evaluator();
        let result = eval.evaluate_value("event.name|trim", &ctx).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn test_transform_contains() {
        let ctx = json!({ "event": { "msg": "critical error occurred" } });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.msg|contains('error')", &ctx)
            .unwrap());
        assert!(!eval
            .evaluate_bool("event.msg|contains('warning')", &ctx)
            .unwrap());
    }

    #[test]
    fn test_transform_starts_with() {
        let ctx = json!({ "event": { "path": "/api/v1/users" } });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.path|startsWith('/api')", &ctx)
            .unwrap());
        assert!(!eval
            .evaluate_bool("event.path|startsWith('/web')", &ctx)
            .unwrap());
    }

    #[test]
    fn test_transform_ends_with() {
        let ctx = json!({ "event": { "file": "report.pdf" } });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.file|endsWith('.pdf')", &ctx)
            .unwrap());
        assert!(!eval
            .evaluate_bool("event.file|endsWith('.txt')", &ctx)
            .unwrap());
    }

    #[test]
    fn test_transform_match() {
        let ctx = json!({ "event": { "msg": "Error: timeout after 30s" } });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.msg|match('timeout')", &ctx)
            .unwrap());
    }

    #[test]
    fn test_transform_length_string() {
        let ctx = json!({ "event": { "name": "hello" } });
        let eval = evaluator();
        let result = eval.evaluate_value("event.name|length", &ctx).unwrap();
        assert_eq!(result, json!(5.0));
    }

    #[test]
    fn test_transform_length_array() {
        let ctx = json!({ "items": ["a", "b", "c"] });
        let eval = evaluator();
        let result = eval.evaluate_value("items|length", &ctx).unwrap();
        assert_eq!(result, json!(3.0));
    }

    #[test]
    fn test_transform_split() {
        let ctx = json!({ "csv": "a,b,c" });
        let eval = evaluator();
        let result = eval.evaluate_value("csv|split(',')", &ctx).unwrap();
        assert_eq!(result, json!(["a", "b", "c"]));
    }

    #[test]
    fn test_transform_not() {
        let ctx = json!({ "event": { "active": true } });
        let eval = evaluator();
        // Use parentheses to apply not transform
        assert!(!eval
            .evaluate_bool("(event.active)|not", &ctx)
            .unwrap());

        let ctx_false = json!({ "event": { "active": false } });
        assert!(eval
            .evaluate_bool("(event.active)|not", &ctx_false)
            .unwrap());
    }

    // -------------------------------------------------------------------
    // Ternary (conditional) expression
    // -------------------------------------------------------------------

    #[test]
    fn test_ternary_expression() {
        let ctx = json!({ "event": { "count": 10.0 } });
        let eval = evaluator();
        let result = eval
            .evaluate_value("(event.count > 5) ? 'high' : 'low'", &ctx)
            .unwrap();
        assert_eq!(result, json!("high"));

        let ctx_low = json!({ "event": { "count": 2.0 } });
        let result = eval
            .evaluate_value("(event.count > 5) ? 'high' : 'low'", &ctx_low)
            .unwrap();
        assert_eq!(result, json!("low"));
    }

    // -------------------------------------------------------------------
    // `in` operator
    // -------------------------------------------------------------------

    #[test]
    fn test_in_operator_array() {
        let ctx = json!({ "event": { "roles": ["admin", "user"] } });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("'admin' in event.roles", &ctx)
            .unwrap());
        assert!(!eval
            .evaluate_bool("'superadmin' in event.roles", &ctx)
            .unwrap());
    }

    // -------------------------------------------------------------------
    // Null handling
    // -------------------------------------------------------------------

    #[test]
    fn test_null_handling() {
        let ctx = json!({ "event": { "name": null } });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool("event.name == null", &ctx)
            .unwrap());

        // Null is falsy
        assert!(!eval.evaluate_bool("event.name", &ctx).unwrap());
    }

    #[test]
    fn test_missing_property_is_null() {
        let ctx = json!({ "event": {} });
        let eval = evaluator();
        // Accessing missing property on an object returns null (not an error)
        let result = eval.evaluate_value("event.nonexistent", &ctx).unwrap();
        assert_eq!(result, json!(null));
    }

    // -------------------------------------------------------------------
    // evaluate_bool edge cases
    // -------------------------------------------------------------------

    #[test]
    fn test_evaluate_bool_truthy_string() {
        let ctx = json!({ "val": "non-empty" });
        let eval = evaluator();
        assert!(eval.evaluate_bool("val", &ctx).unwrap());
    }

    #[test]
    fn test_evaluate_bool_falsy_empty_string() {
        let ctx = json!({ "val": "" });
        let eval = evaluator();
        assert!(!eval.evaluate_bool("val", &ctx).unwrap());
    }

    #[test]
    fn test_evaluate_bool_truthy_number() {
        let ctx = json!({ "val": 42.0 });
        let eval = evaluator();
        assert!(eval.evaluate_bool("val", &ctx).unwrap());
    }

    #[test]
    fn test_evaluate_bool_falsy_zero() {
        let ctx = json!({ "val": 0.0 });
        let eval = evaluator();
        assert!(!eval.evaluate_bool("val", &ctx).unwrap());
    }

    #[test]
    fn test_invalid_context_not_object() {
        let ctx = json!("not an object");
        let eval = evaluator();
        assert!(eval.evaluate_bool("true", &ctx).is_err());
    }

    // -------------------------------------------------------------------
    // WorkflowContext integration
    // -------------------------------------------------------------------

    #[test]
    fn test_evaluate_in_workflow_context_step_output() {
        let eval = evaluator();
        let mut wf_ctx = WorkflowContext {
            step_outputs: std::collections::HashMap::new(),
            trigger_payload: None,
            variables: std::collections::HashMap::new(),
            workflow_name: "test-wf".to_string(),
            run_id: "run-001".to_string(),
        };
        wf_ctx
            .step_outputs
            .insert("gather".to_string(), json!("news articles"));

        assert!(eval
            .evaluate_in_workflow_context(
                "steps.gather.output == 'news articles'",
                &wf_ctx,
            )
            .unwrap());
    }

    #[test]
    fn test_evaluate_in_workflow_context_trigger() {
        let eval = evaluator();
        let wf_ctx = WorkflowContext {
            step_outputs: std::collections::HashMap::new(),
            trigger_payload: Some(json!({ "source": "github", "event": "push" })),
            variables: std::collections::HashMap::new(),
            workflow_name: "test-wf".to_string(),
            run_id: "run-001".to_string(),
        };

        assert!(eval
            .evaluate_in_workflow_context("trigger.source == 'github'", &wf_ctx)
            .unwrap());
    }

    #[test]
    fn test_evaluate_in_workflow_context_variables() {
        let eval = evaluator();
        let mut wf_ctx = WorkflowContext {
            step_outputs: std::collections::HashMap::new(),
            trigger_payload: None,
            variables: std::collections::HashMap::new(),
            workflow_name: "test-wf".to_string(),
            run_id: "run-001".to_string(),
        };
        wf_ctx
            .variables
            .insert("max_retries".to_string(), json!(5.0));

        assert!(eval
            .evaluate_in_workflow_context("variables.max_retries > 3", &wf_ctx)
            .unwrap());
    }

    #[test]
    fn test_evaluate_in_workflow_context_workflow_metadata() {
        let eval = evaluator();
        let wf_ctx = WorkflowContext {
            step_outputs: std::collections::HashMap::new(),
            trigger_payload: None,
            variables: std::collections::HashMap::new(),
            workflow_name: "daily-digest".to_string(),
            run_id: "run-123".to_string(),
        };

        assert!(eval
            .evaluate_in_workflow_context(
                "workflow.name == 'daily-digest'",
                &wf_ctx,
            )
            .unwrap());
    }

    // -------------------------------------------------------------------
    // Complex real-world expressions
    // -------------------------------------------------------------------

    #[test]
    fn test_complex_webhook_filter() {
        let ctx = json!({
            "event": {
                "source": "github",
                "action": "push",
                "branch": "main",
                "author": "alice"
            }
        });
        let eval = evaluator();
        assert!(eval
            .evaluate_bool(
                "event.source == 'github' && event.action == 'push' && event.branch == 'main'",
                &ctx,
            )
            .unwrap());
    }

    #[test]
    fn test_transform_chaining() {
        let ctx = json!({ "name": "  Hello World  " });
        let eval = evaluator();
        let result = eval.evaluate_value("name|trim|lower", &ctx).unwrap();
        assert_eq!(result, json!("hello world"));
    }

    #[test]
    fn test_length_comparison() {
        let ctx = json!({ "items": ["a", "b", "c", "d", "e"] });
        let eval = evaluator();
        assert!(eval.evaluate_bool("items|length > 3", &ctx).unwrap());
        assert!(!eval.evaluate_bool("items|length > 10", &ctx).unwrap());
    }
}
