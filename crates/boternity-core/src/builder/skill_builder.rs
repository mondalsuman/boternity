//! SkillBuilder -- generates SKILL.md manifests from natural language descriptions.
//!
//! Stateless utility (no fields, provider passed per-call) following the
//! pattern established in 02-06. Supports both LLM-driven generation via
//! `generate_skill()` and heuristic capability suggestion via
//! `suggest_capabilities()`.

use serde::{Deserialize, Serialize};

use boternity_types::llm::{
    CompletionRequest, Message, MessageRole, OutputConfig, OutputFormat, OutputJsonSchema,
};

use crate::llm::box_provider::BoxLlmProvider;
use crate::skill::manifest::{parse_skill_md, validate_manifest};

use super::agent::BuilderError;

// ---------------------------------------------------------------------------
// Request / result types
// ---------------------------------------------------------------------------

/// What kind of skill to generate.
#[derive(Debug, Clone)]
pub enum SkillBuildType {
    /// Prompt-based (system prompt injection, no compiled code).
    Local,
    /// WASM-based (generates Rust source code for the WIT interface).
    Wasm { language: String },
}

impl Default for SkillBuildType {
    fn default() -> Self {
        Self::Wasm {
            language: "rust".to_owned(),
        }
    }
}

/// Input to `SkillBuilder::generate_skill`.
#[derive(Debug, Clone)]
pub struct SkillBuildRequest {
    pub name: String,
    pub description: String,
    pub skill_type: SkillBuildType,
    /// Optional pre-selected capabilities (overrides LLM suggestion).
    pub capabilities: Option<Vec<String>>,
}

/// A single capability suggestion with a human-readable reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedCapability {
    pub capability: String,
    pub reason: String,
}

/// The full result of a skill build operation.
#[derive(Debug, Clone)]
pub struct SkillBuildResult {
    pub manifest: boternity_types::skill::SkillManifest,
    pub skill_md_content: String,
    pub source_code: Option<String>,
    pub suggested_capabilities: Vec<SuggestedCapability>,
}

// ---------------------------------------------------------------------------
// LLM response schema (internal, not exported)
// ---------------------------------------------------------------------------

/// JSON shape the LLM is constrained to produce.
#[derive(Debug, Deserialize)]
struct SkillGenerationResponse {
    skill_md: String,
    capabilities: Vec<CapabilityEntry>,
    source_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CapabilityEntry {
    capability: String,
    reason: String,
}

// ---------------------------------------------------------------------------
// SkillBuilder
// ---------------------------------------------------------------------------

/// Stateless utility that generates skills from natural language descriptions.
///
/// Two modes:
/// - **LLM-driven** (`generate_skill`): Full SKILL.md + optional source code.
/// - **Heuristic** (`suggest_capabilities`): Fast keyword-based capability
///   suggestions for inline use in the builder flow.
pub struct SkillBuilder;

impl SkillBuilder {
    /// Generate a complete skill from a natural-language description via LLM.
    ///
    /// The LLM produces a SKILL.md manifest and optional Rust source code
    /// (for WASM skills). The response is parsed and validated before returning.
    pub async fn generate_skill(
        provider: &BoxLlmProvider,
        request: &SkillBuildRequest,
    ) -> Result<SkillBuildResult, BuilderError> {
        let skill_type_str = match &request.skill_type {
            SkillBuildType::Local => "prompt (local)",
            SkillBuildType::Wasm { language } => language.as_str(),
        };

        let caps_hint = match &request.capabilities {
            Some(caps) => format!("Required capabilities: {}", caps.join(", ")),
            None => "Suggest appropriate capabilities based on the description.".to_owned(),
        };

        let source_code_instruction = match &request.skill_type {
            SkillBuildType::Local => {
                "This is a prompt-based skill. Set source_code to null."
            }
            SkillBuildType::Wasm { .. } => {
                "Generate Rust source code implementing the boternity:skill/execute WIT interface. \
                 The code should define a Guest struct that implements the execute function. \
                 Set source_code to the full Rust source."
            }
        };

        let system_prompt = format!(
            r#"<skill_builder_instructions>
Generate a SKILL.md manifest following the agentskills.io specification.

The manifest must include YAML frontmatter between --- delimiters with:
- name: "{name}" (slug format: lowercase, hyphens, no spaces)
- description: clear, concise description
- metadata:
    version: "0.1.0"
    skill-type: {skill_type}
    capabilities: list of required permissions (use snake_case: read_file, write_file, http_get, http_post, exec_command, read_env, recall_memory, get_secret)
    author: "builder"
    categories: relevant categories

The markdown body after frontmatter should contain:
- A heading with the skill name
- A description section
- Usage instructions for the agent

{caps_hint}

{source_code_instruction}

Output as a JSON object with exactly these fields:
- skill_md: the full SKILL.md content (frontmatter + body)
- capabilities: array of objects with "capability" and "reason" fields
- source_code: Rust source code string if WASM type, null if prompt type
</skill_builder_instructions>"#,
            name = request.name,
            skill_type = skill_type_str,
        );

        let user_message = format!(
            "Create a skill called \"{}\" that does the following: {}",
            request.name, request.description,
        );

        // Build structured output schema for the response.
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "skill_md": { "type": "string" },
                "capabilities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "capability": { "type": "string" },
                            "reason": { "type": "string" }
                        },
                        "required": ["capability", "reason"],
                        "additionalProperties": false
                    }
                },
                "source_code": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                }
            },
            "required": ["skill_md", "capabilities", "source_code"],
            "additionalProperties": false
        });

        let completion_request = CompletionRequest {
            model: String::new(), // provider fills model
            messages: vec![Message {
                role: MessageRole::User,
                content: user_message,
            }],
            system: Some(system_prompt),
            max_tokens: 4096,
            temperature: Some(0.4),
            stream: false,
            stop_sequences: None,
            output_config: Some(OutputConfig {
                format: OutputFormat {
                    type_field: "json_schema".to_owned(),
                    json_schema: OutputJsonSchema {
                        name: "skill_generation".to_owned(),
                        schema,
                        strict: Some(true),
                    },
                },
            }),
        };

        let response = provider
            .complete(&completion_request)
            .await
            .map_err(|e| BuilderError::LlmError(e.to_string()))?;

        // Parse the structured response.
        let generated: SkillGenerationResponse = serde_json::from_str(&response.content)
            .map_err(|e| BuilderError::ParseError(format!("Failed to parse LLM response: {e}")))?;

        // Parse and validate the generated SKILL.md.
        let (manifest, _body) = parse_skill_md(&generated.skill_md)
            .map_err(|e| BuilderError::ParseError(format!("Generated SKILL.md is invalid: {e}")))?;

        let suggested_capabilities = generated
            .capabilities
            .into_iter()
            .map(|c| SuggestedCapability {
                capability: c.capability,
                reason: c.reason,
            })
            .collect();

        Ok(SkillBuildResult {
            manifest,
            skill_md_content: generated.skill_md,
            source_code: generated.source_code,
            suggested_capabilities,
        })
    }

    /// Fast, heuristic capability suggestion based on keyword matching.
    ///
    /// Returns capabilities with reasons without calling the LLM. Useful for
    /// inline suggestions during the builder wizard.
    pub fn suggest_capabilities(description: &str) -> Vec<SuggestedCapability> {
        let lower = description.to_lowercase();
        let mut suggestions = Vec::new();

        // Network-related keywords
        if lower.contains("fetch")
            || lower.contains("http")
            || lower.contains("web")
            || lower.contains("api")
            || lower.contains("url")
            || lower.contains("request")
        {
            suggestions.push(SuggestedCapability {
                capability: "http_get".to_owned(),
                reason: "Needs to make HTTP requests to fetch data".to_owned(),
            });
        }

        // File read keywords
        if lower.contains("file")
            || lower.contains("read")
            || lower.contains("disk")
            || lower.contains("load")
        {
            suggestions.push(SuggestedCapability {
                capability: "read_file".to_owned(),
                reason: "Needs to read files from disk".to_owned(),
            });
        }

        // File write keywords
        if lower.contains("write") || lower.contains("save") || lower.contains("output") {
            suggestions.push(SuggestedCapability {
                capability: "write_file".to_owned(),
                reason: "Needs to write or save files to disk".to_owned(),
            });
        }

        // Environment variable keywords
        if lower.contains("environment")
            || lower.contains("env")
            || lower.contains("config")
        {
            suggestions.push(SuggestedCapability {
                capability: "read_env".to_owned(),
                reason: "Needs to read environment variables or configuration".to_owned(),
            });
        }

        // Database keywords -> network access
        if lower.contains("database")
            || lower.contains("sql")
            || lower.contains("query")
        {
            suggestions.push(SuggestedCapability {
                capability: "http_get".to_owned(),
                reason: "Database connections require network access".to_owned(),
            });
        }

        // Command execution keywords
        if lower.contains("execute")
            || lower.contains("command")
            || lower.contains("shell")
            || lower.contains("process")
            || lower.contains("run")
        {
            suggestions.push(SuggestedCapability {
                capability: "exec_command".to_owned(),
                reason: "Needs to execute system commands".to_owned(),
            });
        }

        // Secret/key access
        if lower.contains("secret")
            || lower.contains("api key")
            || lower.contains("token")
            || lower.contains("credential")
        {
            suggestions.push(SuggestedCapability {
                capability: "get_secret".to_owned(),
                reason: "Needs to access secrets or API keys".to_owned(),
            });
        }

        // Memory access
        if lower.contains("remember")
            || lower.contains("memory")
            || lower.contains("recall")
        {
            suggestions.push(SuggestedCapability {
                capability: "recall_memory".to_owned(),
                reason: "Needs to access bot memory for context".to_owned(),
            });
        }

        // Deduplicate by capability name (first match wins)
        let mut seen = std::collections::HashSet::new();
        suggestions.retain(|s| seen.insert(s.capability.clone()));

        suggestions
    }

    /// Validate a SKILL.md string and return a list of warnings.
    ///
    /// Returns an empty list if the manifest is valid. This is structural
    /// validation only (no LLM call). The `provider` parameter is reserved
    /// for future semantic validation.
    pub async fn validate_skill(
        _provider: &BoxLlmProvider,
        skill_md: &str,
    ) -> Result<Vec<String>, BuilderError> {
        let mut warnings = Vec::new();

        // Attempt to parse the SKILL.md
        let (manifest, body) = match parse_skill_md(skill_md) {
            Ok(result) => result,
            Err(e) => {
                warnings.push(format!("Parse error: {e}"));
                return Ok(warnings);
            }
        };

        // Validate the manifest structure
        if let Err(e) = validate_manifest(&manifest) {
            warnings.push(format!("Manifest validation: {e}"));
        }

        // Additional heuristic warnings
        if body.trim().is_empty() {
            warnings.push(
                "SKILL.md body is empty; consider adding usage instructions".to_owned(),
            );
        }

        if manifest.metadata.is_none() {
            warnings.push(
                "No metadata section; consider adding version, skill-type, and capabilities"
                    .to_owned(),
            );
        }

        if let Some(ref meta) = manifest.metadata {
            if meta.version.is_none() {
                warnings.push("No version in metadata; consider adding version: \"0.1.0\"".to_owned());
            }
            if meta.skill_type.is_none() {
                warnings.push(
                    "No skill-type in metadata; consider specifying 'prompt' or 'tool'"
                        .to_owned(),
                );
            }
        }

        Ok(warnings)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_capabilities_network_access() {
        let suggestions = SkillBuilder::suggest_capabilities("fetch data from API");
        assert!(
            suggestions
                .iter()
                .any(|s| s.capability == "http_get"),
            "Expected http_get for 'fetch data from API', got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_file_read() {
        let suggestions = SkillBuilder::suggest_capabilities("read config files from disk");
        assert!(
            suggestions.iter().any(|s| s.capability == "read_file"),
            "Expected read_file for 'read config files', got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_file_write() {
        let suggestions = SkillBuilder::suggest_capabilities("save results to a file");
        assert!(
            suggestions.iter().any(|s| s.capability == "write_file"),
            "Expected write_file for 'save results to a file', got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_env_access() {
        let suggestions = SkillBuilder::suggest_capabilities("read environment variables");
        assert!(
            suggestions.iter().any(|s| s.capability == "read_env"),
            "Expected read_env for 'read environment variables', got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_database_needs_network() {
        let suggestions = SkillBuilder::suggest_capabilities("query a database");
        assert!(
            suggestions
                .iter()
                .any(|s| s.capability == "http_get"
                    && s.reason.to_lowercase().contains("database")),
            "Expected http_get with database reason, got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_exec_command() {
        let suggestions = SkillBuilder::suggest_capabilities("execute shell commands");
        assert!(
            suggestions
                .iter()
                .any(|s| s.capability == "exec_command"),
            "Expected exec_command for 'execute shell commands', got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_empty_for_simple_description() {
        let suggestions = SkillBuilder::suggest_capabilities("greet the user warmly");
        assert!(
            suggestions.is_empty(),
            "Expected no capabilities for 'greet the user warmly', got: {suggestions:?}"
        );
    }

    #[test]
    fn suggest_capabilities_deduplicates() {
        // "fetch web api" triggers http_get from multiple keywords
        let suggestions = SkillBuilder::suggest_capabilities("fetch web api data");
        let http_get_count = suggestions
            .iter()
            .filter(|s| s.capability == "http_get")
            .count();
        assert_eq!(
            http_get_count, 1,
            "Expected exactly 1 http_get, got {http_get_count}"
        );
    }

    #[test]
    fn suggest_capabilities_has_reasons() {
        let suggestions = SkillBuilder::suggest_capabilities("fetch data from web API");
        for s in &suggestions {
            assert!(
                !s.reason.is_empty(),
                "Capability '{}' has empty reason",
                s.capability
            );
        }
    }

    #[tokio::test]
    async fn validate_skill_valid_manifest() {
        // Create a minimal mock provider (validate_skill doesn't call LLM)
        let provider = create_noop_provider();

        let skill_md = r#"---
name: hello-world
description: A simple greeting skill
metadata:
  version: "0.1.0"
  skill-type: prompt
---

# Hello World

Greet the user warmly.
"#;

        let warnings = SkillBuilder::validate_skill(&provider, skill_md)
            .await
            .unwrap();
        assert!(
            warnings.is_empty(),
            "Expected no warnings for valid SKILL.md, got: {warnings:?}"
        );
    }

    #[tokio::test]
    async fn validate_skill_invalid_no_frontmatter() {
        let provider = create_noop_provider();

        let skill_md = "# Just a heading\n\nNo frontmatter here.";

        let warnings = SkillBuilder::validate_skill(&provider, skill_md)
            .await
            .unwrap();
        assert!(
            !warnings.is_empty(),
            "Expected warnings for missing frontmatter"
        );
        assert!(
            warnings[0].contains("Parse error"),
            "Expected parse error warning, got: {}",
            warnings[0]
        );
    }

    #[tokio::test]
    async fn validate_skill_empty_body_warns() {
        let provider = create_noop_provider();

        let skill_md = "---\nname: empty-body\ndescription: Missing body\nmetadata:\n  version: \"0.1.0\"\n  skill-type: prompt\n---\n";

        let warnings = SkillBuilder::validate_skill(&provider, skill_md)
            .await
            .unwrap();
        assert!(
            warnings.iter().any(|w| w.contains("body is empty")),
            "Expected empty body warning, got: {warnings:?}"
        );
    }

    #[tokio::test]
    async fn validate_skill_no_metadata_warns() {
        let provider = create_noop_provider();

        let skill_md = "---\nname: no-meta\ndescription: No metadata\n---\n\nSome instructions.";

        let warnings = SkillBuilder::validate_skill(&provider, skill_md)
            .await
            .unwrap();
        assert!(
            warnings.iter().any(|w| w.contains("No metadata")),
            "Expected no-metadata warning, got: {warnings:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Noop provider helper (validate_skill doesn't call LLM)
    // -----------------------------------------------------------------------

    fn create_noop_provider() -> BoxLlmProvider {
        use std::pin::Pin;

        use futures_util::Stream;

        use boternity_types::llm::{
            CompletionResponse, LlmError, ProviderCapabilities, StreamEvent, TokenCount,
        };

        use crate::llm::provider::LlmProvider;

        struct NoopProvider;

        impl LlmProvider for NoopProvider {
            fn name(&self) -> &str {
                "noop"
            }

            fn capabilities(&self) -> &ProviderCapabilities {
                static CAPS: ProviderCapabilities = ProviderCapabilities {
                    streaming: false,
                    tool_calling: false,
                    vision: false,
                    extended_thinking: false,
                    max_context_tokens: 0,
                    max_output_tokens: 0,
                };
                &CAPS
            }

            async fn complete(
                &self,
                _request: &CompletionRequest,
            ) -> Result<CompletionResponse, LlmError> {
                Err(LlmError::Provider {
                    message: "noop provider".to_owned(),
                })
            }

            fn stream(
                &self,
                _request: CompletionRequest,
            ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>
            {
                Box::pin(futures_util::stream::empty())
            }

            async fn count_tokens(
                &self,
                _request: &CompletionRequest,
            ) -> Result<TokenCount, LlmError> {
                Ok(TokenCount { input_tokens: 0 })
            }
        }

        BoxLlmProvider::new(NoopProvider)
    }
}
