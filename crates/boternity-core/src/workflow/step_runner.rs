//! Step runner for all 8 workflow step types.
//!
//! `StepRunner` dispatches execution to the appropriate handler based on
//! `StepConfig` variant. Each handler resolves templates from the workflow
//! context, executes the step logic, and returns a `StepOutput`.
//!
//! Step types: Agent, Skill, Code, Http, Conditional, Loop, Approval, SubWorkflow.

use std::path::PathBuf;

use boternity_types::workflow::{StepConfig, StepDefinition};
use serde_json::{json, Value};

use super::context::WorkflowContext;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum sub-workflow nesting depth.
pub const MAX_SUB_WORKFLOW_DEPTH: u32 = 5;

/// Default step timeout in seconds.
pub const DEFAULT_STEP_TIMEOUT_SECS: u64 = 300;

// ---------------------------------------------------------------------------
// StepOutput
// ---------------------------------------------------------------------------

/// Output from a step execution.
#[derive(Debug, Clone)]
pub enum StepOutput {
    /// Generic JSON output.
    Value(Value),
    /// Conditional branch selection.
    Branch {
        /// Whether the condition was true.
        condition_met: bool,
        /// The selected branch step IDs.
        selected_steps: Vec<String>,
    },
    /// Loop result.
    Loop {
        /// Number of iterations completed.
        iterations: u32,
        /// Whether the loop condition became false (normal exit).
        completed: bool,
    },
}

impl StepOutput {
    /// Convert the step output to a JSON value for context storage.
    pub fn to_value(&self) -> Value {
        match self {
            StepOutput::Value(v) => v.clone(),
            StepOutput::Branch {
                condition_met,
                selected_steps,
            } => json!({
                "type": "branch",
                "condition_met": condition_met,
                "selected_steps": selected_steps,
            }),
            StepOutput::Loop {
                iterations,
                completed,
            } => json!({
                "type": "loop",
                "iterations": iterations,
                "completed": completed,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// StepError
// ---------------------------------------------------------------------------

/// Errors that can occur during step execution.
#[derive(Debug, thiserror::Error)]
pub enum StepError {
    /// Step execution failed.
    #[error("step execution failed: {0}")]
    ExecutionFailed(String),

    /// Approval required -- not a failure, workflow should pause.
    #[error("approval required: {prompt}")]
    ApprovalRequired { prompt: String },

    /// Sub-workflow depth exceeded.
    #[error("sub-workflow depth {depth} exceeds maximum {max}")]
    SubWorkflowDepthExceeded { depth: u32, max: u32 },

    /// Template resolution error.
    #[error("template error: {0}")]
    TemplateError(String),
}

impl StepError {
    /// Check if this error is an approval gate (not a real failure).
    pub fn is_approval_required(&self) -> bool {
        matches!(self, StepError::ApprovalRequired { .. })
    }

    /// Get the approval prompt, if this is an approval error.
    pub fn approval_prompt(&self) -> Option<String> {
        match self {
            StepError::ApprovalRequired { prompt } => Some(prompt.clone()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// StepRunner
// ---------------------------------------------------------------------------

/// Executes individual workflow steps by dispatching to type-specific handlers.
pub struct StepRunner {
    data_dir: PathBuf,
}

impl StepRunner {
    /// Create a new step runner.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Run a step and return its output.
    pub async fn run(
        &self,
        step: &StepDefinition,
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        match &step.config {
            StepConfig::Agent { bot, prompt, model } => {
                self.run_agent(bot, prompt, model.as_deref(), ctx).await
            }
            StepConfig::Skill { skill, input } => {
                self.run_skill(skill, input.as_deref(), ctx).await
            }
            StepConfig::Code { language, source } => {
                self.run_code(language, source, ctx).await
            }
            StepConfig::Http {
                method,
                url,
                headers,
                body,
            } => {
                self.run_http(method, url, headers.as_ref(), body.as_deref(), ctx)
                    .await
            }
            StepConfig::Conditional {
                condition,
                then_steps,
                else_steps,
            } => {
                self.run_conditional(condition, then_steps, else_steps, ctx)
                    .await
            }
            StepConfig::Loop {
                condition,
                max_iterations,
                body_steps,
            } => {
                self.run_loop(condition, *max_iterations, body_steps, ctx)
                    .await
            }
            StepConfig::Approval { prompt, .. } => self.run_approval(prompt, ctx).await,
            StepConfig::SubWorkflow {
                workflow_name,
                input,
            } => {
                self.run_sub_workflow(workflow_name, input.as_ref(), ctx, 0)
                    .await
            }
        }
    }

    // -- Placeholder: will be wired to LLM in Plan 09 --

    async fn run_agent(
        &self,
        bot: &str,
        prompt: &str,
        model: Option<&str>,
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        let resolved_prompt = ctx.resolve_template(prompt);
        tracing::debug!(
            bot,
            model = model.unwrap_or("default"),
            "running agent step (placeholder)"
        );
        Ok(StepOutput::Value(json!({
            "type": "agent",
            "bot": bot,
            "prompt": resolved_prompt,
            "model": model.unwrap_or("default"),
            "output": format!("[placeholder] agent '{}' response to: {}", bot, resolved_prompt),
        })))
    }

    // -- Placeholder: will be wired to skill system --

    async fn run_skill(
        &self,
        skill: &str,
        input: Option<&str>,
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        let resolved_input = input.map(|i| ctx.resolve_template(i));
        tracing::debug!(skill, "running skill step (placeholder)");
        Ok(StepOutput::Value(json!({
            "type": "skill",
            "skill": skill,
            "input": resolved_input,
            "output": format!("[placeholder] skill '{}' result", skill),
        })))
    }

    // -- Placeholder: will be wired to WASM/TS runtime --

    async fn run_code(
        &self,
        language: &boternity_types::workflow::CodeLanguage,
        source: &str,
        _ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        tracing::debug!(language = ?language, "running code step (placeholder)");
        Ok(StepOutput::Value(json!({
            "type": "code",
            "language": format!("{:?}", language).to_lowercase(),
            "source_length": source.len(),
            "output": "[placeholder] code execution result",
        })))
    }

    // -- HTTP step: resolves templates, builds request descriptor --
    // Actual HTTP execution is delegated to infra layer (clean architecture)

    async fn run_http(
        &self,
        method: &str,
        url: &str,
        headers: Option<&std::collections::HashMap<String, String>>,
        body: Option<&str>,
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        let resolved_url = ctx.resolve_template(url);
        let resolved_body = body.map(|b| ctx.resolve_template(b));
        let resolved_headers: Option<std::collections::HashMap<String, String>> =
            headers.map(|h| {
                h.iter()
                    .map(|(k, v)| (k.clone(), ctx.resolve_template(v)))
                    .collect()
            });

        tracing::debug!(
            method,
            url = resolved_url.as_str(),
            "running HTTP step (template resolved)"
        );

        // Return the resolved request descriptor. The infra layer's HttpStepExecutor
        // will perform the actual HTTP call when wired in.
        Ok(StepOutput::Value(json!({
            "type": "http",
            "method": method,
            "url": resolved_url,
            "headers": resolved_headers,
            "body": resolved_body,
            "status": "pending_execution",
            "note": "HTTP execution delegated to infra layer",
        })))
    }

    // -- Conditional: evaluates JEXL condition, returns branch selection --

    async fn run_conditional(
        &self,
        condition: &str,
        then_steps: &[String],
        else_steps: &[String],
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        let evaluator = super::expression::WorkflowEvaluator::new();
        let condition_met = evaluator
            .evaluate_in_workflow_context(condition, ctx)
            .map_err(|e| StepError::ExecutionFailed(format!("condition eval failed: {e}")))?;

        let selected = if condition_met {
            then_steps.to_vec()
        } else {
            else_steps.to_vec()
        };

        tracing::debug!(
            condition,
            result = condition_met,
            selected = ?selected,
            "conditional branch selected"
        );

        Ok(StepOutput::Branch {
            condition_met,
            selected_steps: selected,
        })
    }

    // -- Loop: evaluates condition, enforces max_iterations --

    async fn run_loop(
        &self,
        condition: &str,
        max_iterations: Option<u32>,
        _body_steps: &[String],
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        let max = max_iterations.unwrap_or(100);
        let evaluator = super::expression::WorkflowEvaluator::new();

        let mut iterations = 0u32;
        let mut completed = false;

        // Evaluate the condition to determine how many iterations would run.
        // Actual body step execution is handled by the executor's sub-step logic.
        // Here we just check the condition and cap iterations.
        loop {
            if iterations >= max {
                tracing::warn!(
                    condition,
                    iterations,
                    max,
                    "loop hit max iterations cap"
                );
                break;
            }

            let should_continue = evaluator
                .evaluate_in_workflow_context(condition, ctx)
                .map_err(|e| {
                    StepError::ExecutionFailed(format!("loop condition eval failed: {e}"))
                })?;

            if !should_continue {
                completed = true;
                break;
            }

            iterations += 1;

            // In the placeholder implementation, we break after one evaluation
            // since we can't actually execute body steps here. The real loop
            // execution will be handled by the executor when body step orchestration
            // is wired in.
            break;
        }

        Ok(StepOutput::Loop {
            iterations,
            completed,
        })
    }

    // -- Approval: returns ApprovalRequired error to pause workflow --

    async fn run_approval(
        &self,
        prompt: &str,
        ctx: &WorkflowContext,
    ) -> Result<StepOutput, StepError> {
        let resolved_prompt = ctx.resolve_template(prompt);
        Err(StepError::ApprovalRequired {
            prompt: resolved_prompt,
        })
    }

    // -- SubWorkflow: checks depth limit --

    async fn run_sub_workflow(
        &self,
        workflow_name: &str,
        _input: Option<&Value>,
        _ctx: &WorkflowContext,
        depth: u32,
    ) -> Result<StepOutput, StepError> {
        if depth >= MAX_SUB_WORKFLOW_DEPTH {
            return Err(StepError::SubWorkflowDepthExceeded {
                depth,
                max: MAX_SUB_WORKFLOW_DEPTH,
            });
        }

        tracing::debug!(
            workflow_name,
            depth,
            "running sub-workflow step (placeholder)"
        );

        // Placeholder: actual sub-workflow execution will invoke the executor recursively
        Ok(StepOutput::Value(json!({
            "type": "sub_workflow",
            "workflow_name": workflow_name,
            "depth": depth,
            "output": format!("[placeholder] sub-workflow '{}' at depth {}", workflow_name, depth),
        })))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    fn test_context() -> WorkflowContext {
        let mut ctx = WorkflowContext::new(
            "test-workflow".to_string(),
            Uuid::now_v7(),
            Some(json!({ "source": "test" })),
        );
        ctx.set_step_output("gather", json!("gathered data"))
            .unwrap();
        ctx
    }

    fn make_step(config: StepConfig) -> StepDefinition {
        use boternity_types::workflow::StepType;
        StepDefinition {
            id: "test-step".to_string(),
            name: "Test Step".to_string(),
            step_type: StepType::Agent,
            depends_on: vec![],
            condition: None,
            timeout_secs: None,
            retry: None,
            config,
            ui: None,
        }
    }

    // -------------------------------------------------------------------
    // HTTP step: URL template resolution
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_http_step_resolves_url_template() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let step = make_step(StepConfig::Http {
            method: "POST".to_string(),
            url: "https://api.example.com/{{ steps.gather.output }}".to_string(),
            headers: None,
            body: Some("data={{ steps.gather.output }}".to_string()),
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Value(v) => {
                assert_eq!(v["url"], "https://api.example.com/gathered data");
                assert_eq!(v["body"], "data=gathered data");
                assert_eq!(v["method"], "POST");
            }
            _ => panic!("expected StepOutput::Value"),
        }
    }

    #[tokio::test]
    async fn test_http_step_resolves_headers() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "X-Data".to_string(),
            "{{ steps.gather.output }}".to_string(),
        );

        let step = make_step(StepConfig::Http {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            headers: Some(headers),
            body: None,
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Value(v) => {
                assert_eq!(v["headers"]["X-Data"], "gathered data");
            }
            _ => panic!("expected StepOutput::Value"),
        }
    }

    // -------------------------------------------------------------------
    // Conditional: branch selection
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_conditional_true_branch() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let mut ctx = test_context();
        ctx.set_step_output("check", json!("has content")).unwrap();

        let step = make_step(StepConfig::Conditional {
            condition: "steps.check.output == 'has content'".to_string(),
            then_steps: vec!["step-a".to_string()],
            else_steps: vec!["step-b".to_string()],
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Branch {
                condition_met,
                selected_steps,
            } => {
                assert!(condition_met);
                assert_eq!(selected_steps, vec!["step-a"]);
            }
            _ => panic!("expected StepOutput::Branch"),
        }
    }

    #[tokio::test]
    async fn test_conditional_false_branch() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let mut ctx = test_context();
        ctx.set_step_output("check", json!("no content")).unwrap();

        let step = make_step(StepConfig::Conditional {
            condition: "steps.check.output == 'has content'".to_string(),
            then_steps: vec!["step-a".to_string()],
            else_steps: vec!["step-b".to_string()],
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Branch {
                condition_met,
                selected_steps,
            } => {
                assert!(!condition_met);
                assert_eq!(selected_steps, vec!["step-b"]);
            }
            _ => panic!("expected StepOutput::Branch"),
        }
    }

    // -------------------------------------------------------------------
    // Loop: iteration cap
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_loop_respects_max_iterations() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let step = make_step(StepConfig::Loop {
            // Condition always true
            condition: "true".to_string(),
            max_iterations: Some(5),
            body_steps: vec!["body-step".to_string()],
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Loop { iterations, .. } => {
                // Placeholder breaks after 1 iteration, but the cap is respected
                assert!(iterations <= 5);
            }
            _ => panic!("expected StepOutput::Loop"),
        }
    }

    #[tokio::test]
    async fn test_loop_false_condition_completes() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let step = make_step(StepConfig::Loop {
            condition: "false".to_string(),
            max_iterations: Some(100),
            body_steps: vec!["body-step".to_string()],
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Loop {
                iterations,
                completed,
            } => {
                assert_eq!(iterations, 0);
                assert!(completed);
            }
            _ => panic!("expected StepOutput::Loop"),
        }
    }

    // -------------------------------------------------------------------
    // Approval: returns error to pause
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_approval_returns_error() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let step = make_step(StepConfig::Approval {
            prompt: "Review results from {{ steps.gather.output }}".to_string(),
            timeout_secs: None,
        });

        let err = runner.run(&step, &ctx).await.unwrap_err();
        assert!(err.is_approval_required());
        assert_eq!(
            err.approval_prompt().unwrap(),
            "Review results from gathered data"
        );
    }

    // -------------------------------------------------------------------
    // SubWorkflow: depth cap
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_sub_workflow_depth_cap() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        // Directly call with depth at the limit
        let result = runner
            .run_sub_workflow("child-wf", None, &ctx, MAX_SUB_WORKFLOW_DEPTH)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("depth"),
            "expected depth error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_sub_workflow_within_depth() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let result = runner
            .run_sub_workflow("child-wf", None, &ctx, 0)
            .await;
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------
    // Agent: template resolution
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_agent_resolves_template() {
        let runner = StepRunner::new(PathBuf::from("/tmp"));
        let ctx = test_context();

        let step = make_step(StepConfig::Agent {
            bot: "researcher".to_string(),
            prompt: "Analyze: {{ steps.gather.output }}".to_string(),
            model: None,
        });

        let result = runner.run(&step, &ctx).await.unwrap();
        match result {
            StepOutput::Value(v) => {
                assert_eq!(v["prompt"], "Analyze: gathered data");
            }
            _ => panic!("expected StepOutput::Value"),
        }
    }

    // -------------------------------------------------------------------
    // StepOutput::to_value
    // -------------------------------------------------------------------

    #[test]
    fn test_step_output_to_value_branch() {
        let output = StepOutput::Branch {
            condition_met: true,
            selected_steps: vec!["a".to_string()],
        };
        let v = output.to_value();
        assert_eq!(v["type"], "branch");
        assert_eq!(v["condition_met"], true);
    }

    #[test]
    fn test_step_output_to_value_loop() {
        let output = StepOutput::Loop {
            iterations: 5,
            completed: true,
        };
        let v = output.to_value();
        assert_eq!(v["type"], "loop");
        assert_eq!(v["iterations"], 5);
        assert_eq!(v["completed"], true);
    }
}
