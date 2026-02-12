//! Multi-provider fallback chain.
//!
//! Stub definitions for the `FallbackChain` struct and its methods.
//! Full routing logic will be implemented in Plan 03-03.

use std::collections::HashMap;

use boternity_types::llm::{FallbackChainConfig, ProviderCostInfo, ProviderStatusInfo};

use super::box_provider::BoxLlmProvider;
use super::health::ProviderHealth;

/// Routes LLM requests through multiple providers with automatic failover.
///
/// Providers are ordered by priority. On failure, the chain tries the next
/// available provider according to circuit breaker state and rate limits.
pub struct FallbackChain {
    /// Provider health trackers paired with their boxed provider instances.
    pub providers: Vec<(ProviderHealth, BoxLlmProvider)>,
    /// Cost information keyed by provider name.
    pub cost_table: HashMap<String, ProviderCostInfo>,
    /// Name of the primary (highest priority) provider.
    pub primary_provider_name: String,
    /// Maximum time (ms) to wait in rate-limit queue before failing over.
    pub rate_limit_queue_timeout_ms: u64,
    /// Warn if fallback provider costs more than this multiplier of the primary.
    pub cost_warning_multiplier: f64,
}

impl FallbackChain {
    /// Create a new fallback chain from configuration and provider instances.
    pub fn new(
        config: FallbackChainConfig,
        providers: Vec<BoxLlmProvider>,
        cost_table: HashMap<String, ProviderCostInfo>,
    ) -> Self {
        let primary_provider_name = config
            .providers
            .iter()
            .min_by_key(|p| p.priority)
            .map(|p| p.name.clone())
            .unwrap_or_default();

        let health_providers = config
            .providers
            .iter()
            .zip(providers)
            .map(|(cfg, provider)| {
                let health = ProviderHealth::new(&cfg.name, cfg.priority);
                (health, provider)
            })
            .collect();

        Self {
            providers: health_providers,
            cost_table,
            primary_provider_name,
            rate_limit_queue_timeout_ms: config.rate_limit_queue_timeout_ms,
            cost_warning_multiplier: config.cost_warning_multiplier,
        }
    }

    /// Get health status of all providers (for CLI `provider status` command).
    pub fn health_status(&self) -> Vec<ProviderStatusInfo> {
        self.providers
            .iter()
            .map(|(health, _)| health.to_status_info())
            .collect()
    }

    /// Send a completion request through the fallback chain.
    ///
    /// Stub -- full implementation in Plan 03-03.
    pub async fn complete(
        &mut self,
        _request: &boternity_types::llm::CompletionRequest,
    ) -> Result<boternity_types::llm::CompletionResponse, boternity_types::llm::LlmError> {
        todo!("FallbackChain::complete will be implemented in Plan 03-03")
    }

    /// Send a streaming completion request through the fallback chain.
    ///
    /// Stub -- full implementation in Plan 03-03.
    pub fn stream(
        &mut self,
        _request: boternity_types::llm::CompletionRequest,
    ) -> std::pin::Pin<
        Box<
            dyn futures_util::Stream<
                    Item = Result<
                        boternity_types::llm::StreamEvent,
                        boternity_types::llm::LlmError,
                    >,
                > + Send
                + 'static,
        >,
    > {
        todo!("FallbackChain::stream will be implemented in Plan 03-03")
    }
}
