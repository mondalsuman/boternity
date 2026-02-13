//! Global configuration loader for Boternity.
//!
//! Reads `config.toml` from the data directory (`~/.boternity/` in production)
//! and deserializes it into [`GlobalConfig`]. Falls back to sensible defaults
//! when the file is missing or malformed.

use std::path::Path;

use boternity_types::config::GlobalConfig;

/// Minimum token budget per request (safety floor).
const MIN_REQUEST_BUDGET: u32 = 10_000;

/// Load global configuration from `{data_dir}/config.toml`.
///
/// - If the file does not exist, returns [`GlobalConfig::default()`] (500,000 token budget).
/// - If the file exists but fails to parse, logs a warning and returns the default.
/// - If the file exists and parses successfully, returns the parsed config.
pub async fn load_global_config(data_dir: &Path) -> GlobalConfig {
    let config_path = data_dir.join("config.toml");

    let content = match tokio::fs::read_to_string(&config_path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!("No config.toml found at {}, using defaults", config_path.display());
            return GlobalConfig::default();
        }
        Err(err) => {
            tracing::warn!("Failed to read {}: {err}, using defaults", config_path.display());
            return GlobalConfig::default();
        }
    };

    match toml::from_str::<GlobalConfig>(&content) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(
                "Failed to parse {}: {err}, using defaults",
                config_path.display()
            );
            GlobalConfig::default()
        }
    }
}

/// Resolve the per-request token budget.
///
/// Priority:
/// 1. Per-bot override from IDENTITY.md frontmatter (`max_request_tokens` field)
/// 2. Global default from `config.toml` (`default_request_budget`)
///
/// A minimum floor of 10,000 tokens is enforced regardless of source.
pub fn resolve_request_budget(global_config: &GlobalConfig, identity_override: Option<u32>) -> u32 {
    let budget = identity_override.unwrap_or(global_config.default_request_budget);
    budget.max(MIN_REQUEST_BUDGET)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn load_global_config_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let config = load_global_config(tmp.path()).await;
        assert_eq!(config.default_request_budget, 500_000);
        assert!(config.provider_pricing.is_empty());
    }

    #[tokio::test]
    async fn load_global_config_valid_toml_returns_parsed() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        tokio::fs::write(
            &config_path,
            r#"
default_request_budget = 1000000

[[provider_pricing]]
provider_name = "anthropic"
model_pattern = "claude-sonnet-*"
input_cost_per_million = 3.0
output_cost_per_million = 15.0
"#,
        )
        .await
        .unwrap();

        let config = load_global_config(tmp.path()).await;
        assert_eq!(config.default_request_budget, 1_000_000);
        assert_eq!(config.provider_pricing.len(), 1);
        assert_eq!(config.provider_pricing[0].provider_name, "anthropic");
    }

    #[tokio::test]
    async fn load_global_config_invalid_toml_returns_default() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        tokio::fs::write(&config_path, "this is not { valid toml !!!")
            .await
            .unwrap();

        let config = load_global_config(tmp.path()).await;
        assert_eq!(config.default_request_budget, 500_000);
        assert!(config.provider_pricing.is_empty());
    }

    #[test]
    fn resolve_request_budget_with_identity_override() {
        let global = GlobalConfig {
            default_request_budget: 500_000,
            provider_pricing: Vec::new(),
        };
        let budget = resolve_request_budget(&global, Some(200_000));
        assert_eq!(budget, 200_000);
    }

    #[test]
    fn resolve_request_budget_without_override_uses_global() {
        let global = GlobalConfig {
            default_request_budget: 750_000,
            provider_pricing: Vec::new(),
        };
        let budget = resolve_request_budget(&global, None);
        assert_eq!(budget, 750_000);
    }

    #[test]
    fn resolve_request_budget_enforces_minimum() {
        let global = GlobalConfig {
            default_request_budget: 500,
            provider_pricing: Vec::new(),
        };
        // Global below minimum
        assert_eq!(resolve_request_budget(&global, None), MIN_REQUEST_BUDGET);
        // Identity override below minimum
        assert_eq!(
            resolve_request_budget(&global, Some(5_000)),
            MIN_REQUEST_BUDGET
        );
    }
}
