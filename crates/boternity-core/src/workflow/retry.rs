//! Retry handler with Simple and LLM self-correction strategies.
//!
//! Provides stateless retry logic for workflow step execution. Two strategies:
//! - **Simple**: re-execute the step with identical inputs up to `max_attempts`.
//! - **LLM Self-Correct**: feed the error back to an LLM agent that analyzes
//!   the failure and suggests a corrected approach before re-execution.

use boternity_types::workflow::{RetryConfig, RetryStrategy, StepConfig, StepDefinition};

// ---------------------------------------------------------------------------
// RetryAction
// ---------------------------------------------------------------------------

/// The action to take when retrying a failed step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryAction {
    /// Re-run the step with the same inputs (Simple strategy).
    Rerun,
    /// Ask an LLM to analyze the failure and produce a corrected prompt.
    SelfCorrect {
        /// The prompt to send to the LLM for self-correction analysis.
        analysis_prompt: String,
    },
}

// ---------------------------------------------------------------------------
// RetryHandler
// ---------------------------------------------------------------------------

/// Stateless retry handler for workflow step failures.
///
/// Same pattern as `MemoryExtractor` -- no internal state, all logic is in
/// associated functions that take configuration as parameters.
pub struct RetryHandler;

impl RetryHandler {
    /// Determine whether a retry should be attempted.
    ///
    /// Returns `true` if `attempt` is less than `config.max_attempts`.
    /// `attempt` is 1-based (first execution is attempt 1).
    pub fn should_retry(config: &RetryConfig, attempt: u32, _error: &str) -> bool {
        attempt < config.max_attempts
    }

    /// Prepare the retry action for a failed step.
    ///
    /// - **Simple**: returns `RetryAction::Rerun`.
    /// - **LlmSelfCorrect**: builds an analysis prompt containing the step
    ///   details, error message, and attempt count, then returns
    ///   `RetryAction::SelfCorrect`.
    pub fn prepare_retry(
        config: &RetryConfig,
        step: &StepDefinition,
        error: &str,
        _context: &super::context::WorkflowContext,
    ) -> RetryAction {
        match config.strategy {
            RetryStrategy::Simple => RetryAction::Rerun,
            RetryStrategy::LlmSelfCorrect => {
                let analysis_prompt = Self::build_self_correct_prompt(
                    &step.name,
                    &step.config,
                    error,
                    // We don't know the exact attempt number in prepare_retry,
                    // but the caller tracks it. Use 0 as placeholder -- callers
                    // typically call should_retry first to get the attempt count.
                    0,
                    config.max_attempts,
                );
                RetryAction::SelfCorrect { analysis_prompt }
            }
        }
    }

    /// Build the self-correction analysis prompt for LLM retry.
    ///
    /// The prompt instructs the LLM to analyze why the step failed and
    /// suggest a different approach for the next attempt.
    pub fn build_self_correct_prompt(
        step_name: &str,
        step_config: &StepConfig,
        error: &str,
        attempt: u32,
        max_attempts: u32,
    ) -> String {
        let config_summary = Self::summarize_step_config(step_config);
        let remaining = max_attempts.saturating_sub(attempt + 1);

        format!(
            "## Workflow Step Self-Correction Analysis\n\
             \n\
             A workflow step has failed and needs your help to determine a better approach.\n\
             \n\
             **Step:** {step_name}\n\
             **Configuration:** {config_summary}\n\
             **Attempt:** {attempt_display} of {max_attempts} ({remaining} remaining)\n\
             **Error:**\n\
             ```\n\
             {error}\n\
             ```\n\
             \n\
             Please analyze this failure and suggest a corrected approach. Consider:\n\
             1. What went wrong in the previous attempt?\n\
             2. What should be different in the next attempt?\n\
             3. Is there a fundamental issue that retrying won't fix?\n\
             \n\
             Provide a concise corrected instruction or approach for the next attempt.",
            attempt_display = attempt + 1,
        )
    }

    /// Produce a human-readable summary of a step's configuration.
    fn summarize_step_config(config: &StepConfig) -> String {
        match config {
            StepConfig::Agent { bot, prompt, model } => {
                let model_str = model.as_deref().unwrap_or("default");
                format!("Agent step (bot={bot}, model={model_str}, prompt={prompt:?})")
            }
            StepConfig::Skill { skill, input } => {
                let input_str = input.as_deref().unwrap_or("none");
                format!("Skill step (skill={skill}, input={input_str:?})")
            }
            StepConfig::Code { language, source } => {
                let lang = format!("{:?}", language).to_lowercase();
                let preview = if source.len() > 60 {
                    format!("{}...", &source[..60])
                } else {
                    source.clone()
                };
                format!("Code step (language={lang}, source={preview:?})")
            }
            StepConfig::Http { method, url, .. } => {
                format!("HTTP step ({method} {url})")
            }
            StepConfig::Conditional { condition, .. } => {
                format!("Conditional step (condition={condition:?})")
            }
            StepConfig::Loop { condition, max_iterations, .. } => {
                let max = max_iterations.map_or("unlimited".to_string(), |m| m.to_string());
                format!("Loop step (condition={condition:?}, max_iterations={max})")
            }
            StepConfig::Approval { prompt, .. } => {
                format!("Approval step (prompt={prompt:?})")
            }
            StepConfig::SubWorkflow { workflow_name, .. } => {
                format!("SubWorkflow step (workflow={workflow_name})")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::workflow::{RetryConfig, RetryStrategy, StepConfig, StepDefinition, StepType};

    fn make_agent_step(name: &str) -> StepDefinition {
        StepDefinition {
            id: name.to_lowercase().replace(' ', "-"),
            name: name.to_string(),
            step_type: StepType::Agent,
            depends_on: vec![],
            condition: None,
            timeout_secs: None,
            retry: None,
            config: StepConfig::Agent {
                bot: "test-bot".to_string(),
                prompt: "Do something useful".to_string(),
                model: None,
            },
            ui: None,
        }
    }

    fn make_workflow_context() -> super::super::context::WorkflowContext {
        super::super::context::WorkflowContext::new(
            "test-workflow".to_string(),
            uuid::Uuid::now_v7(),
            None,
        )
    }

    // -------------------------------------------------------------------
    // should_retry
    // -------------------------------------------------------------------

    #[test]
    fn test_should_retry_within_limit() {
        let config = RetryConfig {
            max_attempts: 3,
            strategy: RetryStrategy::Simple,
        };
        assert!(RetryHandler::should_retry(&config, 1, "error"));
        assert!(RetryHandler::should_retry(&config, 2, "error"));
    }

    #[test]
    fn test_should_not_retry_at_max() {
        let config = RetryConfig {
            max_attempts: 3,
            strategy: RetryStrategy::Simple,
        };
        assert!(!RetryHandler::should_retry(&config, 3, "error"));
    }

    #[test]
    fn test_should_not_retry_beyond_max() {
        let config = RetryConfig {
            max_attempts: 3,
            strategy: RetryStrategy::Simple,
        };
        assert!(!RetryHandler::should_retry(&config, 4, "error"));
    }

    #[test]
    fn test_should_retry_single_attempt() {
        let config = RetryConfig {
            max_attempts: 1,
            strategy: RetryStrategy::Simple,
        };
        // With max_attempts=1, attempt 1 is the only one; no retry
        assert!(!RetryHandler::should_retry(&config, 1, "error"));
    }

    // -------------------------------------------------------------------
    // prepare_retry -- Simple
    // -------------------------------------------------------------------

    #[test]
    fn test_prepare_retry_simple_returns_rerun() {
        let config = RetryConfig {
            max_attempts: 3,
            strategy: RetryStrategy::Simple,
        };
        let step = make_agent_step("Gather News");
        let ctx = make_workflow_context();

        let action = RetryHandler::prepare_retry(&config, &step, "timeout", &ctx);
        assert_eq!(action, RetryAction::Rerun);
    }

    // -------------------------------------------------------------------
    // prepare_retry -- LlmSelfCorrect
    // -------------------------------------------------------------------

    #[test]
    fn test_prepare_retry_llm_self_correct_returns_self_correct() {
        let config = RetryConfig {
            max_attempts: 3,
            strategy: RetryStrategy::LlmSelfCorrect,
        };
        let step = make_agent_step("Analyze Trends");
        let ctx = make_workflow_context();

        let action = RetryHandler::prepare_retry(&config, &step, "LLM returned empty response", &ctx);
        match &action {
            RetryAction::SelfCorrect { analysis_prompt } => {
                assert!(analysis_prompt.contains("Analyze Trends"));
                assert!(analysis_prompt.contains("LLM returned empty response"));
                assert!(analysis_prompt.contains("Self-Correction"));
            }
            _ => panic!("Expected SelfCorrect action, got {:?}", action),
        }
    }

    // -------------------------------------------------------------------
    // build_self_correct_prompt
    // -------------------------------------------------------------------

    #[test]
    fn test_build_self_correct_prompt_contains_details() {
        let config = StepConfig::Agent {
            bot: "researcher".to_string(),
            prompt: "Find top news".to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
        };

        let prompt = RetryHandler::build_self_correct_prompt(
            "Gather News",
            &config,
            "Connection timeout after 30s",
            1,
            3,
        );

        assert!(prompt.contains("Gather News"), "Should contain step name");
        assert!(prompt.contains("Connection timeout after 30s"), "Should contain error");
        assert!(prompt.contains("2 of 3"), "Should show attempt 2 of 3");
        assert!(prompt.contains("1 remaining"), "Should show remaining attempts");
        assert!(prompt.contains("Agent step"), "Should describe step type");
        assert!(prompt.contains("researcher"), "Should include bot name");
    }

    #[test]
    fn test_build_self_correct_prompt_http_step() {
        let config = StepConfig::Http {
            method: "POST".to_string(),
            url: "https://api.example.com/data".to_string(),
            headers: None,
            body: None,
        };

        let prompt = RetryHandler::build_self_correct_prompt(
            "Send Data",
            &config,
            "HTTP 503 Service Unavailable",
            0,
            2,
        );

        assert!(prompt.contains("Send Data"));
        assert!(prompt.contains("HTTP step"));
        assert!(prompt.contains("POST https://api.example.com/data"));
        assert!(prompt.contains("503 Service Unavailable"));
    }

    #[test]
    fn test_build_self_correct_prompt_last_attempt() {
        let config = StepConfig::Agent {
            bot: "bot".to_string(),
            prompt: "test".to_string(),
            model: None,
        };

        let prompt = RetryHandler::build_self_correct_prompt("Step", &config, "error", 2, 3);
        assert!(prompt.contains("3 of 3"), "Should show last attempt");
        assert!(prompt.contains("0 remaining"), "Should show 0 remaining");
    }

    // -------------------------------------------------------------------
    // Max 3 attempts default enforcement
    // -------------------------------------------------------------------

    #[test]
    fn test_default_max_attempts_is_three() {
        // Verify through YAML deserialization that default is 3
        let yaml = "strategy: simple";
        let config: RetryConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.max_attempts, 3);

        // With default of 3, attempts 1 and 2 should retry, 3 should not
        assert!(RetryHandler::should_retry(&config, 1, "error"));
        assert!(RetryHandler::should_retry(&config, 2, "error"));
        assert!(!RetryHandler::should_retry(&config, 3, "error"));
    }
}
