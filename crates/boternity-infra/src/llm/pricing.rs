//! Cost estimation and pricing for LLM providers.
//!
//! Provides a hardcoded default pricing table for known models with
//! user override capability from `config.toml`. Cost estimates are
//! clearly labeled as approximate (`~$0.12`).

use boternity_types::config::ProviderPricing;

/// Internal pricing entry for the hardcoded default table.
struct PricingEntry {
    provider: &'static str,
    model_pattern: &'static str,
    input_cost_per_million: f64,
    output_cost_per_million: f64,
}

/// Conservative fallback pricing when no model match is found.
const FALLBACK_INPUT_COST: f64 = 5.0;
const FALLBACK_OUTPUT_COST: f64 = 15.0;

/// Return the hardcoded default pricing table for known providers/models.
///
/// Prices are approximate as of early 2026 and expressed in USD per million tokens.
fn default_pricing_table() -> Vec<PricingEntry> {
    vec![
        // Anthropic
        PricingEntry {
            provider: "anthropic",
            model_pattern: "claude-sonnet-4",
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
        },
        PricingEntry {
            provider: "anthropic",
            model_pattern: "claude-opus-4",
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
        },
        PricingEntry {
            provider: "anthropic",
            model_pattern: "claude-haiku-3",
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
        },
        // Bedrock (pass-through Anthropic pricing)
        PricingEntry {
            provider: "bedrock",
            model_pattern: "claude-sonnet-4",
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
        },
        PricingEntry {
            provider: "bedrock",
            model_pattern: "claude-opus-4",
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
        },
        PricingEntry {
            provider: "bedrock",
            model_pattern: "claude-haiku-3",
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
        },
        // OpenAI
        PricingEntry {
            provider: "openai",
            model_pattern: "gpt-4o-mini",
            input_cost_per_million: 0.15,
            output_cost_per_million: 0.60,
        },
        PricingEntry {
            provider: "openai",
            model_pattern: "gpt-4o",
            input_cost_per_million: 2.50,
            output_cost_per_million: 10.0,
        },
        // Google
        PricingEntry {
            provider: "google",
            model_pattern: "gemini-2",
            input_cost_per_million: 1.25,
            output_cost_per_million: 5.0,
        },
        // Mistral
        PricingEntry {
            provider: "mistral",
            model_pattern: "mistral-large",
            input_cost_per_million: 2.0,
            output_cost_per_million: 6.0,
        },
    ]
}

/// Check if a model name matches a pattern using simple prefix matching.
///
/// The pattern is treated as a prefix: `"claude-sonnet-4"` matches
/// `"claude-sonnet-4-20250514"`, `"claude-sonnet-4.5"`, etc.
fn matches_pattern(model: &str, pattern: &str) -> bool {
    model.starts_with(pattern)
}

/// Estimate the cost of a request in USD.
///
/// Lookup order:
/// 1. User-defined pricing overrides from `config.toml`
/// 2. Hardcoded default pricing table
/// 3. Conservative fallback ($5.00 / $15.00 per million tokens)
///
/// The model is matched by prefix against available patterns. For Bedrock
/// models that include region prefixes (e.g., `eu.anthropic.claude-sonnet-4-...`),
/// the full model string is checked, so Bedrock entries in the pricing table
/// use the base model name which matches the suffix after the provider prefix.
pub fn estimate_cost(
    input_tokens: u32,
    output_tokens: u32,
    model: &str,
    provider: &str,
    user_pricing: &[ProviderPricing],
) -> f64 {
    // 1. Check user overrides first
    for pricing in user_pricing {
        if pricing.provider_name == provider && matches_pattern(model, &pricing.model_pattern) {
            return compute_cost(
                input_tokens,
                output_tokens,
                pricing.input_cost_per_million,
                pricing.output_cost_per_million,
            );
        }
    }

    // 2. Check default pricing table
    let defaults = default_pricing_table();

    // For bedrock models like "eu.anthropic.claude-sonnet-4-...", try matching
    // with the provider from the table, checking if model contains the pattern
    for entry in &defaults {
        if entry.provider == provider && matches_pattern(model, entry.model_pattern) {
            return compute_cost(
                input_tokens,
                output_tokens,
                entry.input_cost_per_million,
                entry.output_cost_per_million,
            );
        }
    }

    // For bedrock, also try matching the model name without the region prefix
    // e.g., "eu.anthropic.claude-sonnet-4-20250929-v1:0" -> check if it contains the pattern
    if provider == "bedrock" {
        for entry in &defaults {
            if entry.provider == "bedrock" && model.contains(entry.model_pattern) {
                return compute_cost(
                    input_tokens,
                    output_tokens,
                    entry.input_cost_per_million,
                    entry.output_cost_per_million,
                );
            }
        }
    }

    // 3. Conservative fallback
    compute_cost(input_tokens, output_tokens, FALLBACK_INPUT_COST, FALLBACK_OUTPUT_COST)
}

/// Compute cost in USD given token counts and per-million rates.
fn compute_cost(
    input_tokens: u32,
    output_tokens: u32,
    input_cost_per_million: f64,
    output_cost_per_million: f64,
) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * input_cost_per_million;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * output_cost_per_million;
    input_cost + output_cost
}

/// Format a cost estimate as a human-readable string.
///
/// Always prefixed with `~` to indicate the value is an estimate.
/// - Costs below $0.01 use 3 decimal places: `~$0.001`
/// - Costs $0.01 and above use 2 decimal places: `~$0.12`
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("~${cost:.3}")
    } else {
        format!("~${cost:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_cost_known_model_returns_correct_value() {
        // claude-sonnet-4: $3.00 input, $15.00 output per million
        let cost = estimate_cost(1_000_000, 100_000, "claude-sonnet-4-20250514", "anthropic", &[]);
        // $3.00 + $1.50 = $4.50
        assert!((cost - 4.50).abs() < 0.001, "Expected ~$4.50, got ${cost}");
    }

    #[test]
    fn estimate_cost_user_override_takes_priority() {
        let user_pricing = vec![ProviderPricing {
            provider_name: "anthropic".to_string(),
            model_pattern: "claude-sonnet-4".to_string(),
            input_cost_per_million: 1.0,
            output_cost_per_million: 5.0,
        }];
        let cost = estimate_cost(1_000_000, 100_000, "claude-sonnet-4-20250514", "anthropic", &user_pricing);
        // $1.00 + $0.50 = $1.50
        assert!((cost - 1.50).abs() < 0.001, "Expected ~$1.50, got ${cost}");
    }

    #[test]
    fn estimate_cost_unknown_model_uses_fallback() {
        let cost = estimate_cost(1_000_000, 100_000, "some-unknown-model", "unknown-provider", &[]);
        // Fallback: $5.00 input + $1.50 output = $6.50
        let expected = (1_000_000.0 / 1_000_000.0) * 5.0 + (100_000.0 / 1_000_000.0) * 15.0;
        assert!((cost - expected).abs() < 0.001, "Expected ${expected}, got ${cost}");
    }

    #[test]
    fn format_cost_small_amounts_three_decimal_places() {
        assert_eq!(format_cost(0.001), "~$0.001");
        assert_eq!(format_cost(0.0054), "~$0.005");
        assert_eq!(format_cost(0.0), "~$0.000");
    }

    #[test]
    fn format_cost_normal_amounts_two_decimal_places() {
        assert_eq!(format_cost(0.12), "~$0.12");
        assert_eq!(format_cost(1.50), "~$1.50");
        assert_eq!(format_cost(4.50), "~$4.50");
    }

    #[test]
    fn default_pricing_table_covers_major_providers() {
        let table = default_pricing_table();
        let providers: Vec<&str> = table.iter().map(|e| e.provider).collect();
        assert!(providers.contains(&"anthropic"), "Missing anthropic");
        assert!(providers.contains(&"openai"), "Missing openai");
        assert!(providers.contains(&"google"), "Missing google");
        assert!(providers.contains(&"mistral"), "Missing mistral");
        assert!(providers.contains(&"bedrock"), "Missing bedrock");
    }

    #[test]
    fn estimate_cost_bedrock_model_with_region_prefix() {
        // Bedrock models have region prefix: "eu.anthropic.claude-sonnet-4-20250929-v1:0"
        let cost = estimate_cost(
            1_000_000,
            100_000,
            "eu.anthropic.claude-sonnet-4-20250929-v1:0",
            "bedrock",
            &[],
        );
        // Should match bedrock/claude-sonnet-4 via contains: $3.00 + $1.50 = $4.50
        assert!((cost - 4.50).abs() < 0.001, "Expected ~$4.50, got ${cost}");
    }

    #[test]
    fn estimate_cost_openai_mini_matches_before_regular() {
        // gpt-4o-mini should match the mini entry, not the gpt-4o entry
        let cost = estimate_cost(1_000_000, 1_000_000, "gpt-4o-mini-2024", "openai", &[]);
        // mini: $0.15 + $0.60 = $0.75
        assert!((cost - 0.75).abs() < 0.001, "Expected ~$0.75, got ${cost}");
    }

    #[test]
    fn estimate_cost_opus_pricing() {
        let cost = estimate_cost(500_000, 50_000, "claude-opus-4-20250514", "anthropic", &[]);
        // $7.50 + $3.75 = $11.25
        let expected = (500_000.0 / 1_000_000.0) * 15.0 + (50_000.0 / 1_000_000.0) * 75.0;
        assert!((cost - expected).abs() < 0.001, "Expected ${expected}, got ${cost}");
    }
}
