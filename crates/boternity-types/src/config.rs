//! Global configuration types for Boternity.
//!
//! `GlobalConfig` represents the top-level `config.toml` that controls
//! request budgets, provider pricing, and other global settings.

use serde::{Deserialize, Serialize};

/// Top-level configuration for the Boternity platform.
///
/// Loaded from `~/.boternity/config.toml`. All fields have sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default token budget per user request (across all sub-agents).
    #[serde(default = "default_request_budget")]
    pub default_request_budget: u32,

    /// Pricing information for cost estimation per provider/model.
    #[serde(default)]
    pub provider_pricing: Vec<ProviderPricing>,
}

fn default_request_budget() -> u32 {
    500_000
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_request_budget: default_request_budget(),
            provider_pricing: Vec::new(),
        }
    }
}

/// Cost information for a specific provider/model pattern.
///
/// Used by the budget tracker to estimate spend and warn about cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    /// Name of the provider (e.g., "anthropic", "openai").
    pub provider_name: String,
    /// Glob-like pattern for matching model names (e.g., "claude-sonnet-*").
    pub model_pattern: String,
    /// Cost per million input tokens in USD.
    pub input_cost_per_million: f64,
    /// Cost per million output tokens in USD.
    pub output_cost_per_million: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_config_default_values() {
        let config = GlobalConfig::default();
        assert_eq!(config.default_request_budget, 500_000);
        assert!(config.provider_pricing.is_empty());
    }

    #[test]
    fn test_global_config_deserialize_with_defaults() {
        let toml_str = "";
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_request_budget, 500_000);
        assert!(config.provider_pricing.is_empty());
    }

    #[test]
    fn test_global_config_deserialize_with_values() {
        let toml_str = r#"
default_request_budget = 1000000

[[provider_pricing]]
provider_name = "anthropic"
model_pattern = "claude-sonnet-*"
input_cost_per_million = 3.0
output_cost_per_million = 15.0

[[provider_pricing]]
provider_name = "openai"
model_pattern = "gpt-4o*"
input_cost_per_million = 2.5
output_cost_per_million = 10.0
"#;
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_request_budget, 1_000_000);
        assert_eq!(config.provider_pricing.len(), 2);
        assert_eq!(config.provider_pricing[0].provider_name, "anthropic");
        assert!((config.provider_pricing[0].input_cost_per_million - 3.0).abs() < f64::EPSILON);
        assert_eq!(config.provider_pricing[1].provider_name, "openai");
    }

    #[test]
    fn test_global_config_serde_roundtrip() {
        let config = GlobalConfig {
            default_request_budget: 750_000,
            provider_pricing: vec![ProviderPricing {
                provider_name: "anthropic".to_string(),
                model_pattern: "claude-*".to_string(),
                input_cost_per_million: 3.0,
                output_cost_per_million: 15.0,
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: GlobalConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_request_budget, 750_000);
        assert_eq!(parsed.provider_pricing.len(), 1);
    }

    #[test]
    fn test_provider_pricing_serde_roundtrip() {
        let pricing = ProviderPricing {
            provider_name: "anthropic".to_string(),
            model_pattern: "claude-opus-*".to_string(),
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
        };
        let json = serde_json::to_string(&pricing).unwrap();
        let parsed: ProviderPricing = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider_name, "anthropic");
        assert_eq!(parsed.model_pattern, "claude-opus-*");
        assert!((parsed.input_cost_per_million - 15.0).abs() < f64::EPSILON);
        assert!((parsed.output_cost_per_million - 75.0).abs() < f64::EPSILON);
    }
}
