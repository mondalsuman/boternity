//! Provider management CLI commands: status, add, remove, list.
//!
//! Provides `bnity provider` subcommand for configuring, monitoring,
//! and managing LLM providers in the multi-provider fallback chain.
//!
//! Provider configurations are persisted in `~/.boternity/providers.json`
//! and loaded on startup to build the fallback chain.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;

use boternity_infra::llm::{create_provider, test_provider_connection};
use boternity_types::llm::{
    ProviderCapabilities, ProviderConfig, ProviderStatusInfo, ProviderType,
};
use boternity_types::secret::SecretScope;

use crate::state::AppState;

/// Provider management subcommands.
#[derive(Subcommand)]
pub enum ProviderCommand {
    /// Show health status of all configured providers.
    Status,

    /// Add a new LLM provider to the fallback chain.
    Add {
        /// Provider name (e.g., "openai", "gemini", "mistral", "glm", "bedrock", "claude_subscription").
        #[arg(long)]
        name: String,

        /// Provider type: anthropic, openai_compatible, bedrock, claude_subscription.
        #[arg(long, value_name = "TYPE")]
        provider_type: String,

        /// Model name (e.g., gpt-4o, gemini-2.5-pro).
        #[arg(long)]
        model: String,

        /// Priority in fallback chain (lower = higher priority).
        #[arg(long, default_value = "10")]
        priority: u32,

        /// Secret name in vault for the API key (e.g., "OPENAI_API_KEY").
        #[arg(long)]
        secret: Option<String>,

        /// Custom base URL override.
        #[arg(long)]
        base_url: Option<String>,

        /// Required flag for experimental providers (e.g., claude_subscription).
        #[arg(long)]
        experimental: bool,

        /// Skip connection test.
        #[arg(long)]
        skip_test: bool,
    },

    /// Remove a provider from the fallback chain.
    Remove {
        /// Provider name to remove.
        name: String,
    },

    /// List all providers in the fallback chain with priority order.
    List,
}

/// Handle a provider management subcommand.
pub async fn handle_provider_command(
    cmd: ProviderCommand,
    state: &AppState,
    json: bool,
) -> Result<()> {
    match cmd {
        ProviderCommand::Status => provider_status(state, json).await,
        ProviderCommand::Add {
            name,
            provider_type,
            model,
            priority,
            secret,
            base_url,
            experimental,
            skip_test,
        } => {
            provider_add(
                state,
                &name,
                &provider_type,
                &model,
                priority,
                secret.as_deref(),
                base_url,
                experimental,
                skip_test,
                json,
            )
            .await
        }
        ProviderCommand::Remove { name } => provider_remove(state, &name, json).await,
        ProviderCommand::List => provider_list(state, json).await,
    }
}

/// Display health status of all configured providers.
///
/// Shows circuit breaker state, last error, uptime, call counts,
/// and failure counts in a formatted table.
async fn provider_status(state: &AppState, json: bool) -> Result<()> {
    let configs = load_provider_configs(&state.data_dir).await?;

    if configs.is_empty() {
        if json {
            println!("[]");
        } else {
            println!();
            println!(
                "  {} No providers configured. Use {} to add one.",
                style("i").blue().bold(),
                style("bnity provider add").cyan()
            );
            println!();
        }
        return Ok(());
    }

    // Build a temporary fallback chain to get live health status.
    // Note: In a running chat session the health is tracked in-memory.
    // For the CLI status command we show the configured providers with
    // their default (healthy) state, since circuit breaker state is
    // only meaningful during an active session.
    let statuses: Vec<ProviderStatusInfo> = configs
        .iter()
        .map(|c| ProviderStatusInfo {
            name: c.name.clone(),
            circuit_state: "closed".to_string(),
            last_error: None,
            last_success_ago: None,
            total_calls: 0,
            total_failures: 0,
            uptime_since: Some(chrono::Utc::now().to_rfc3339()),
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
        return Ok(());
    }

    println!();
    println!("  {}", style("Provider Health Status").bold());
    println!();

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Priority").fg(Color::White),
        Cell::new("Provider").fg(Color::White),
        Cell::new("Type").fg(Color::White),
        Cell::new("Model").fg(Color::White),
        Cell::new("Circuit").fg(Color::White),
        Cell::new("Last Error").fg(Color::White),
        Cell::new("Uptime").fg(Color::White),
        Cell::new("Calls").fg(Color::White),
        Cell::new("Failures").fg(Color::White),
    ]);

    for (config, status) in configs.iter().zip(statuses.iter()) {
        let circuit_cell = match status.circuit_state.as_str() {
            "closed" => Cell::new("closed").fg(Color::Green),
            "open" => Cell::new("OPEN").fg(Color::Red),
            "half_open" => Cell::new("half_open").fg(Color::Yellow),
            other => Cell::new(other).fg(Color::White),
        };

        let last_error = status
            .last_error
            .as_deref()
            .unwrap_or("-")
            .to_string();
        let last_error_display = if last_error.len() > 40 {
            format!("{}...", &last_error[..37])
        } else {
            last_error
        };

        let uptime = status
            .uptime_since
            .as_deref()
            .map(|_| "up".to_string())
            .unwrap_or_else(|| "down".to_string());

        table.add_row(vec![
            Cell::new(config.priority).fg(Color::Cyan),
            Cell::new(&config.name).fg(Color::White),
            Cell::new(config.provider_type.to_string()).fg(Color::DarkGrey),
            Cell::new(&config.model).fg(Color::DarkGrey),
            circuit_cell,
            Cell::new(last_error_display).fg(Color::DarkGrey),
            Cell::new(uptime).fg(Color::Green),
            Cell::new(status.total_calls).fg(Color::White),
            Cell::new(status.total_failures).fg(Color::White),
        ]);
    }

    println!("{table}");
    println!();
    println!(
        "  {} provider{}",
        style(configs.len()).bold(),
        if configs.len() == 1 { "" } else { "s" }
    );
    println!(
        "  {}",
        style("Circuit breaker state resets each chat session.").dim()
    );
    println!();

    Ok(())
}

/// Add a new provider to the fallback chain.
///
/// Parses the provider type, resolves the API key from the vault,
/// creates the provider, tests the connection, and persists the config.
#[allow(clippy::too_many_arguments)]
async fn provider_add(
    state: &AppState,
    name: &str,
    provider_type_str: &str,
    model: &str,
    priority: u32,
    secret_name: Option<&str>,
    base_url: Option<String>,
    experimental: bool,
    skip_test: bool,
    json: bool,
) -> Result<()> {
    // Parse provider type
    let provider_type: ProviderType = provider_type_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;

    // Claude subscription requires --experimental flag
    if provider_type == ProviderType::ClaudeSubscription && !experimental {
        anyhow::bail!(
            "Claude subscription requires --experimental flag.\n\
             This provider violates Anthropic's ToS and may stop working at any time."
        );
    }

    // Load existing configs
    let mut configs = load_provider_configs(&state.data_dir).await?;

    // Check for duplicate name
    if configs.iter().any(|c| c.name == name) {
        anyhow::bail!("Provider '{}' already exists. Use `bnity provider remove {}` first.", name, name);
    }

    // Resolve API key from vault if secret name provided
    let api_key = if let Some(secret) = secret_name {
        let value = state
            .secret_service
            .get_secret(secret, &SecretScope::Global)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Secret '{}' not found in vault. Set it with: bnity set secret {}",
                    secret,
                    secret
                )
            })?;
        Some(value)
    } else if provider_type == ProviderType::ClaudeSubscription {
        None // Claude subscription doesn't need an API key
    } else {
        anyhow::bail!(
            "API key required. Provide --secret <SECRET_NAME> for the vault key.\n\
             Example: bnity set secret OPENAI_API_KEY --value sk-... && \\\n\
             bnity provider add --name openai --provider-type openai_compatible \\\n\
             --model gpt-4o --secret OPENAI_API_KEY"
        );
    };

    // Infer default capabilities from provider type/name
    let capabilities = infer_capabilities(name, &provider_type);

    let config = ProviderConfig {
        name: name.to_string(),
        provider_type: provider_type.clone(),
        api_key_secret_name: secret_name.map(|s| s.to_string()),
        base_url,
        model: model.to_string(),
        priority,
        enabled: true,
        capabilities,
    };

    // Test connection unless skipped
    if !skip_test {
        if !json {
            print!("  Testing connection to {} ({})... ", style(name).cyan(), model);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }

        let provider = create_provider(&config, api_key.as_deref())?;
        match test_provider_connection(&provider).await {
            Ok(()) => {
                if !json {
                    println!("{}", style("connected").green().bold());
                }
            }
            Err(e) => {
                if json {
                    let err = serde_json::json!({
                        "error": "connection_test_failed",
                        "message": e.to_string(),
                        "provider": name,
                    });
                    println!("{}", serde_json::to_string_pretty(&err)?);
                    return Ok(());
                }

                println!("{}", style("FAILED").red().bold());
                eprintln!(
                    "  {} Connection test failed: {}",
                    style("!").red().bold(),
                    e
                );
                eprintln!(
                    "  {} Provider not added. Use {} to skip the test.",
                    style("Tip:").dim(),
                    style("--skip-test").cyan()
                );
                return Ok(());
            }
        }
    }

    // Save config
    configs.push(config.clone());
    save_provider_configs(&state.data_dir, &configs).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!(
            "  {} Provider '{}' added (priority {}, model: {}).",
            style("+").green().bold(),
            style(name).cyan(),
            priority,
            model
        );
    }

    Ok(())
}

/// Remove a provider from the fallback chain.
async fn provider_remove(state: &AppState, name: &str, json: bool) -> Result<()> {
    let mut configs = load_provider_configs(&state.data_dir).await?;

    let before_len = configs.len();
    configs.retain(|c| c.name != name);

    if configs.len() == before_len {
        if json {
            println!(
                "{}",
                serde_json::json!({"error": "not_found", "provider": name})
            );
        } else {
            println!(
                "  {} Provider '{}' not found.",
                style("?").yellow().bold(),
                style(name).cyan()
            );
        }
        return Ok(());
    }

    save_provider_configs(&state.data_dir, &configs).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({"removed": true, "provider": name})
        );
    } else {
        println!(
            "  {} Provider '{}' removed.",
            style("x").red().bold(),
            style(name).cyan()
        );
    }

    Ok(())
}

/// List all providers in fallback chain order.
async fn provider_list(state: &AppState, json: bool) -> Result<()> {
    let mut configs = load_provider_configs(&state.data_dir).await?;
    configs.sort_by_key(|c| c.priority);

    if json {
        println!("{}", serde_json::to_string_pretty(&configs)?);
        return Ok(());
    }

    if configs.is_empty() {
        println!();
        println!(
            "  {} No providers configured. Use {} to add one.",
            style("i").blue().bold(),
            style("bnity provider add").cyan()
        );
        println!();
        return Ok(());
    }

    println!();
    println!("  {}", style("Fallback Chain Order").bold());
    println!();

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Priority").fg(Color::White),
        Cell::new("Name").fg(Color::White),
        Cell::new("Type").fg(Color::White),
        Cell::new("Model").fg(Color::White),
        Cell::new("Enabled").fg(Color::White),
    ]);

    for config in &configs {
        let enabled_cell = if config.enabled {
            Cell::new("yes").fg(Color::Green)
        } else {
            Cell::new("no").fg(Color::Red)
        };

        table.add_row(vec![
            Cell::new(config.priority).fg(Color::Cyan),
            Cell::new(&config.name).fg(Color::White),
            Cell::new(config.provider_type.to_string()).fg(Color::DarkGrey),
            Cell::new(&config.model).fg(Color::DarkGrey),
            enabled_cell,
        ]);
    }

    println!("{table}");
    println!();
    println!(
        "  {} provider{} in chain",
        style(configs.len()).bold(),
        if configs.len() == 1 { "" } else { "s" }
    );
    println!();

    Ok(())
}

// --- Persistence helpers ---

/// Path to the providers.json file.
fn providers_json_path(data_dir: &Path) -> PathBuf {
    data_dir.join("providers.json")
}

/// Load provider configurations from disk.
///
/// Returns an empty vec if the file doesn't exist.
pub async fn load_provider_configs(data_dir: &Path) -> Result<Vec<ProviderConfig>> {
    let path = providers_json_path(data_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let configs: Vec<ProviderConfig> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(configs)
}

/// Save provider configurations to disk.
async fn save_provider_configs(data_dir: &Path, configs: &[ProviderConfig]) -> Result<()> {
    let path = providers_json_path(data_dir);
    let content = serde_json::to_string_pretty(configs)?;
    tokio::fs::write(&path, content)
        .await
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Infer default capabilities from provider name and type.
fn infer_capabilities(name: &str, provider_type: &ProviderType) -> ProviderCapabilities {
    match provider_type {
        ProviderType::Anthropic | ProviderType::Bedrock => ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false,
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
        },
        ProviderType::ClaudeSubscription => ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: true,
            max_context_tokens: 200_000,
            max_output_tokens: 128_000,
        },
        ProviderType::OpenAiCompatible => match name {
            "openai" => ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 128_000,
                max_output_tokens: 16_384,
            },
            "gemini" => ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 1_000_000,
                max_output_tokens: 65_536,
            },
            "mistral" => ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 128_000,
                max_output_tokens: 32_768,
            },
            "glm" => ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: false,
                extended_thinking: false,
                max_context_tokens: 200_000,
                max_output_tokens: 128_000,
            },
            _ => ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: false,
                extended_thinking: false,
                max_context_tokens: 128_000,
                max_output_tokens: 8_192,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_capabilities_anthropic() {
        let caps = infer_capabilities("anthropic", &ProviderType::Anthropic);
        assert!(caps.streaming);
        assert!(caps.tool_calling);
        assert!(caps.vision);
        assert!(!caps.extended_thinking);
        assert_eq!(caps.max_context_tokens, 200_000);
    }

    #[test]
    fn test_infer_capabilities_openai() {
        let caps = infer_capabilities("openai", &ProviderType::OpenAiCompatible);
        assert_eq!(caps.max_context_tokens, 128_000);
        assert_eq!(caps.max_output_tokens, 16_384);
        assert!(caps.vision);
    }

    #[test]
    fn test_infer_capabilities_gemini() {
        let caps = infer_capabilities("gemini", &ProviderType::OpenAiCompatible);
        assert_eq!(caps.max_context_tokens, 1_000_000);
        assert_eq!(caps.max_output_tokens, 65_536);
    }

    #[test]
    fn test_infer_capabilities_claude_subscription() {
        let caps = infer_capabilities("claude_subscription", &ProviderType::ClaudeSubscription);
        assert!(caps.extended_thinking);
        assert_eq!(caps.max_output_tokens, 128_000);
    }

    #[test]
    fn test_infer_capabilities_unknown_openai_compat() {
        let caps = infer_capabilities("custom-llm", &ProviderType::OpenAiCompatible);
        assert_eq!(caps.max_context_tokens, 128_000);
        assert_eq!(caps.max_output_tokens, 8_192);
        assert!(!caps.vision);
    }
}
