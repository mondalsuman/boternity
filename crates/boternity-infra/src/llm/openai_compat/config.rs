//! Configuration types and per-provider defaults for OpenAI-compatible providers.
//!
//! Each provider that speaks the OpenAI chat completions protocol gets a factory
//! function returning an [`OpenAiCompatConfig`] with the correct base URL,
//! capabilities, and defaults.

use std::collections::HashMap;

use boternity_types::llm::{ProviderCapabilities, ProviderCostInfo};

/// Configuration for an OpenAI-compatible LLM provider.
///
/// Used to construct an [`super::OpenAiCompatibleProvider`].
pub struct OpenAiCompatConfig {
    /// Human-readable provider name (e.g., "openai", "gemini").
    pub provider_name: String,
    /// Base URL for the API (e.g., "https://api.openai.com/v1").
    pub base_url: String,
    /// API key for authentication.
    pub api_key: String,
    /// Model identifier (e.g., "gpt-4o", "gemini-2.5-pro").
    pub model: String,
    /// What this provider supports.
    pub capabilities: ProviderCapabilities,
}

/// OpenAI default configuration.
///
/// Base URL: `https://api.openai.com/v1`
/// Capabilities: streaming, tool calling, vision; 128K context, 16K output.
pub fn openai_defaults(api_key: &str, model: &str) -> OpenAiCompatConfig {
    OpenAiCompatConfig {
        provider_name: "openai".into(),
        base_url: "https://api.openai.com/v1".into(),
        api_key: api_key.into(),
        model: model.into(),
        capabilities: ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false,
            max_context_tokens: 128_000,
            max_output_tokens: 16_384,
        },
    }
}

/// Google Gemini default configuration (OpenAI-compatible beta endpoint).
///
/// Base URL: `https://generativelanguage.googleapis.com/v1beta/openai`
/// Capabilities: streaming, tool calling, vision; 1M context, 64K output.
pub fn gemini_defaults(api_key: &str, model: &str) -> OpenAiCompatConfig {
    OpenAiCompatConfig {
        provider_name: "gemini".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
        api_key: api_key.into(),
        model: model.into(),
        capabilities: ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false,
            max_context_tokens: 1_000_000,
            max_output_tokens: 65_536,
        },
    }
}

/// Mistral AI default configuration.
///
/// Base URL: `https://api.mistral.ai/v1`
/// Capabilities: streaming, tool calling, vision; 128K context, 32K output.
pub fn mistral_defaults(api_key: &str, model: &str) -> OpenAiCompatConfig {
    OpenAiCompatConfig {
        provider_name: "mistral".into(),
        base_url: "https://api.mistral.ai/v1".into(),
        api_key: api_key.into(),
        model: model.into(),
        capabilities: ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false,
            max_context_tokens: 128_000,
            max_output_tokens: 32_768,
        },
    }
}

/// GLM 4.7 (z.ai) default configuration.
///
/// Base URL: `https://api.z.ai/api/paas/v4`
/// Capabilities: streaming, tool calling, no vision; 200K context, 128K output.
pub fn glm_defaults(api_key: &str, model: &str) -> OpenAiCompatConfig {
    OpenAiCompatConfig {
        provider_name: "glm".into(),
        base_url: "https://api.z.ai/api/paas/v4".into(),
        api_key: api_key.into(),
        model: model.into(),
        capabilities: ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false,
            extended_thinking: false,
            max_context_tokens: 200_000,
            max_output_tokens: 128_000,
        },
    }
}

/// Claude.ai subscription proxy default configuration.
///
/// **EXPERIMENTAL:** Requires `claude-max-api-proxy` running at `localhost:3456`.
/// Anthropic actively enforces against this usage as of January 2026.
/// This is a ToS-violating, unreliable path included only for experimentation.
///
/// Base URL: `http://localhost:3456/v1`
/// API key: `dummy-key` (proxy handles auth via Claude Code CLI).
/// Capabilities: streaming, tool calling, vision, extended thinking; 200K context, 128K output.
pub fn claude_subscription_defaults(model: &str) -> OpenAiCompatConfig {
    OpenAiCompatConfig {
        provider_name: "claude_subscription".into(),
        base_url: "http://localhost:3456/v1".into(),
        api_key: "dummy-key".into(),
        model: model.into(),
        capabilities: ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: true,
            max_context_tokens: 200_000,
            max_output_tokens: 128_000,
        },
    }
}

/// Hard-coded cost table for all supported providers and common models.
///
/// Costs are approximate as of February 2026. Updated with version bumps.
/// Used by the fallback chain to warn when a fallback provider costs
/// significantly more than the primary.
pub fn default_cost_table() -> HashMap<String, ProviderCostInfo> {
    let entries = vec![
        ProviderCostInfo {
            provider_name: "anthropic".into(),
            model: "claude-sonnet-4-20250514".into(),
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
        },
        ProviderCostInfo {
            provider_name: "anthropic".into(),
            model: "claude-opus-4-20250514".into(),
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
        },
        ProviderCostInfo {
            provider_name: "openai".into(),
            model: "gpt-4o".into(),
            input_cost_per_million: 2.5,
            output_cost_per_million: 10.0,
        },
        ProviderCostInfo {
            provider_name: "openai".into(),
            model: "o3-mini".into(),
            input_cost_per_million: 1.1,
            output_cost_per_million: 4.4,
        },
        ProviderCostInfo {
            provider_name: "gemini".into(),
            model: "gemini-2.5-pro".into(),
            input_cost_per_million: 1.25,
            output_cost_per_million: 10.0,
        },
        ProviderCostInfo {
            provider_name: "mistral".into(),
            model: "mistral-large-latest".into(),
            input_cost_per_million: 2.0,
            output_cost_per_million: 6.0,
        },
        ProviderCostInfo {
            provider_name: "glm".into(),
            model: "glm-4.7".into(),
            input_cost_per_million: 0.5,
            output_cost_per_million: 2.0,
        },
        ProviderCostInfo {
            provider_name: "bedrock".into(),
            model: "claude-sonnet-4-20250514".into(),
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
        },
    ];

    entries
        .into_iter()
        .map(|e| {
            let key = format!("{}:{}", e.provider_name, e.model);
            (key, e)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_defaults() {
        let config = openai_defaults("sk-test", "gpt-4o");
        assert_eq!(config.provider_name, "openai");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-4o");
        assert!(config.capabilities.streaming);
        assert!(config.capabilities.tool_calling);
        assert!(config.capabilities.vision);
        assert!(!config.capabilities.extended_thinking);
        assert_eq!(config.capabilities.max_context_tokens, 128_000);
        assert_eq!(config.capabilities.max_output_tokens, 16_384);
    }

    #[test]
    fn test_gemini_defaults() {
        let config = gemini_defaults("gemini-key", "gemini-2.5-pro");
        assert_eq!(config.provider_name, "gemini");
        assert!(config.base_url.contains("generativelanguage.googleapis.com"));
        assert_eq!(config.capabilities.max_context_tokens, 1_000_000);
        assert_eq!(config.capabilities.max_output_tokens, 65_536);
    }

    #[test]
    fn test_mistral_defaults() {
        let config = mistral_defaults("mistral-key", "mistral-large-latest");
        assert_eq!(config.provider_name, "mistral");
        assert_eq!(config.base_url, "https://api.mistral.ai/v1");
        assert_eq!(config.capabilities.max_context_tokens, 128_000);
        assert_eq!(config.capabilities.max_output_tokens, 32_768);
    }

    #[test]
    fn test_glm_defaults() {
        let config = glm_defaults("glm-key", "glm-4.7");
        assert_eq!(config.provider_name, "glm");
        assert_eq!(config.base_url, "https://api.z.ai/api/paas/v4");
        assert!(!config.capabilities.vision);
        assert_eq!(config.capabilities.max_context_tokens, 200_000);
        assert_eq!(config.capabilities.max_output_tokens, 128_000);
    }

    #[test]
    fn test_claude_subscription_defaults() {
        let config = claude_subscription_defaults("claude-opus-4-20250514");
        assert_eq!(config.provider_name, "claude_subscription");
        assert_eq!(config.base_url, "http://localhost:3456/v1");
        assert_eq!(config.api_key, "dummy-key");
        assert!(config.capabilities.extended_thinking);
        assert_eq!(config.capabilities.max_context_tokens, 200_000);
        assert_eq!(config.capabilities.max_output_tokens, 128_000);
    }

    #[test]
    fn test_default_cost_table_has_all_providers() {
        let table = default_cost_table();
        assert!(table.contains_key("anthropic:claude-sonnet-4-20250514"));
        assert!(table.contains_key("anthropic:claude-opus-4-20250514"));
        assert!(table.contains_key("openai:gpt-4o"));
        assert!(table.contains_key("openai:o3-mini"));
        assert!(table.contains_key("gemini:gemini-2.5-pro"));
        assert!(table.contains_key("mistral:mistral-large-latest"));
        assert!(table.contains_key("glm:glm-4.7"));
        assert!(table.contains_key("bedrock:claude-sonnet-4-20250514"));
        assert_eq!(table.len(), 8);
    }

    #[test]
    fn test_cost_table_values() {
        let table = default_cost_table();
        let openai = &table["openai:gpt-4o"];
        assert!((openai.input_cost_per_million - 2.5).abs() < f64::EPSILON);
        assert!((openai.output_cost_per_million - 10.0).abs() < f64::EPSILON);
    }
}
