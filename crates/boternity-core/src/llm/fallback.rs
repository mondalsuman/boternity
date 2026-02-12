//! Multi-provider fallback chain.
//!
//! Routes LLM requests through multiple providers with automatic failover.
//! Providers are tried in priority order. Transient errors (provider down,
//! rate limited, overloaded) trigger failover; auth/config errors do not.

use std::collections::HashMap;
use std::pin::Pin;
use std::time::Instant;

use futures_util::Stream;

use boternity_types::llm::{
    CompletionRequest, CompletionResponse, FallbackChainConfig, LlmError, ProviderCostInfo,
    ProviderStatusInfo, StreamEvent,
};

use super::box_provider::BoxLlmProvider;
use super::health::ProviderHealth;

/// Result of a successful completion through the fallback chain.
#[derive(Debug)]
pub struct FallbackResult {
    /// The completion response from the provider.
    pub response: CompletionResponse,
    /// Name of the provider that handled the request.
    pub provider_name: String,
    /// Failover warning message, if the request was handled by a non-primary provider.
    pub failover_warning: Option<String>,
}

/// Result of selecting a provider for streaming.
pub struct StreamSelection {
    /// The stream of events from the selected provider.
    pub stream: Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>,
    /// Name of the provider that is streaming.
    pub provider_name: String,
    /// Failover warning message, if streaming from a non-primary provider.
    pub failover_warning: Option<String>,
}

impl std::fmt::Debug for StreamSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamSelection")
            .field("provider_name", &self.provider_name)
            .field("failover_warning", &self.failover_warning)
            .field("stream", &"<stream>")
            .finish()
    }
}

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

    /// Check if the primary (highest priority) provider is currently available.
    ///
    /// Used for auto-switch-back: when the primary recovers, the next request
    /// will naturally route to it since it has the highest priority.
    pub fn primary_available(&mut self) -> bool {
        for (health, _) in &mut self.providers {
            if health.name == self.primary_provider_name {
                return health.is_available();
            }
        }
        false
    }

    /// Build priority-sorted indices for provider selection.
    ///
    /// Returns indices into `self.providers` sorted by priority (ascending),
    /// with ties broken by last latency (ascending) then name (alphabetical).
    fn sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.providers.len()).collect();
        indices.sort_by(|&a, &b| {
            let ha = &self.providers[a].0;
            let hb = &self.providers[b].0;
            ha.priority
                .cmp(&hb.priority)
                .then_with(|| {
                    let la = ha.last_latency_ms.unwrap_or(u64::MAX);
                    let lb = hb.last_latency_ms.unwrap_or(u64::MAX);
                    la.cmp(&lb)
                })
                .then_with(|| ha.name.cmp(&hb.name))
        });
        indices
    }

    /// Build a failover warning string when a non-primary provider handles the request.
    fn build_failover_warning(&self, used_provider: &str) -> Option<String> {
        if used_provider == self.primary_provider_name {
            return None;
        }

        let mut parts = vec![format!("Switched to {used_provider}")];

        // Check capability downgrade
        let primary_caps = self
            .providers
            .iter()
            .find(|(h, _)| h.name == self.primary_provider_name);
        let used_caps = self
            .providers
            .iter()
            .find(|(h, _)| h.name == used_provider);
        if let (Some((_, primary_p)), Some((_, used_p))) = (primary_caps, used_caps) {
            let pc = primary_p.capabilities();
            let uc = used_p.capabilities();
            if uc.max_context_tokens < pc.max_context_tokens
                || uc.max_output_tokens < pc.max_output_tokens
            {
                parts.push(
                    "Running on a smaller model -- responses may be less detailed".to_string(),
                );
            }
        }

        // Check cost escalation
        if let (Some(primary_cost), Some(used_cost)) = (
            self.cost_table.get(&self.primary_provider_name),
            self.cost_table.get(used_provider),
        ) {
            let primary_avg =
                (primary_cost.input_cost_per_million + primary_cost.output_cost_per_million) / 2.0;
            let used_avg =
                (used_cost.input_cost_per_million + used_cost.output_cost_per_million) / 2.0;
            if primary_avg > 0.0 {
                let ratio = used_avg / primary_avg;
                if ratio > self.cost_warning_multiplier {
                    parts.push(format!(
                        "Note: {used_provider} costs ~{ratio:.1}x more than {}",
                        self.primary_provider_name
                    ));
                }
            }
        }

        Some(parts.join(". "))
    }

    /// Send a completion request through the fallback chain.
    ///
    /// Tries providers in priority order. On transient errors (provider down,
    /// rate limited, overloaded), fails over to the next available provider.
    /// Auth and config errors are returned immediately without failover.
    ///
    /// Returns the response, the name of the provider that handled it, and
    /// an optional failover warning if a non-primary provider was used.
    pub async fn complete(
        &mut self,
        request: &CompletionRequest,
    ) -> Result<FallbackResult, LlmError> {
        let indices = self.sorted_indices();
        let mut last_error: Option<LlmError> = None;

        for idx in indices {
            let (health, _provider) = &mut self.providers[idx];
            let provider_name = health.name.clone();

            // Check if rate-limited but within queue timeout
            if let Some(until) = health.rate_limit_until {
                let now = Instant::now();
                if now < until {
                    let remaining_ms = until.duration_since(now).as_millis() as u64;
                    if remaining_ms <= self.rate_limit_queue_timeout_ms {
                        tracing::debug!(
                            provider = %provider_name,
                            remaining_ms,
                            "Queuing for rate-limited provider"
                        );
                        tokio::time::sleep(until.duration_since(now)).await;
                        // Clear the rate limit after waiting
                        self.providers[idx].0.rate_limit_until = None;
                    }
                }
            }

            // Check availability (circuit breaker + rate limit)
            if !self.providers[idx].0.is_available() {
                tracing::debug!(provider = %provider_name, "Provider unavailable, skipping");
                continue;
            }

            let start = Instant::now();
            let (_health, provider) = &mut self.providers[idx];

            match provider.complete(request).await {
                Ok(response) => {
                    let latency_ms = start.elapsed().as_millis() as u64;
                    self.providers[idx].0.record_success();
                    self.providers[idx].0.last_latency_ms = Some(latency_ms);

                    let failover_warning = self.build_failover_warning(&provider_name);
                    if let Some(ref warning) = failover_warning {
                        tracing::warn!(%warning, "Failover occurred");
                    }

                    return Ok(FallbackResult {
                        response,
                        provider_name,
                        failover_warning,
                    });
                }
                Err(err) => {
                    let latency_ms = start.elapsed().as_millis() as u64;
                    self.providers[idx].0.last_latency_ms = Some(latency_ms);

                    // Non-failover errors: return immediately
                    if !ProviderHealth::is_failover_error(&err) {
                        tracing::error!(
                            provider = %provider_name,
                            error = %err,
                            "Non-failover error, returning immediately"
                        );
                        return Err(err);
                    }

                    // Failover error: record failure and try next
                    tracing::warn!(
                        provider = %provider_name,
                        error = %err,
                        "Provider failed, trying next in chain"
                    );

                    // Handle rate-limited specifically: set the rate limit timer
                    if let LlmError::RateLimited { retry_after_ms } = &err {
                        self.providers[idx]
                            .0
                            .set_rate_limited(*retry_after_ms, self.rate_limit_queue_timeout_ms);
                    }

                    self.providers[idx].0.record_failure(&err);
                    last_error = Some(err);
                }
            }
        }

        // All providers exhausted
        Err(last_error.unwrap_or(LlmError::Provider {
            message: "All providers in fallback chain are unavailable. Run `bnity provider status` for details.".to_string(),
        }))
    }

    /// Select a provider for streaming and return its stream.
    ///
    /// Selects the first available provider by priority and starts its stream.
    /// Mid-stream failover is not possible -- if the stream errors after starting,
    /// the error is propagated to the caller.
    ///
    /// Returns the stream along with provider name and optional failover warning.
    pub fn select_stream(
        &mut self,
        request: CompletionRequest,
    ) -> Result<StreamSelection, LlmError> {
        let indices = self.sorted_indices();

        for idx in indices {
            if !self.providers[idx].0.is_available() {
                let name = &self.providers[idx].0.name;
                tracing::debug!(provider = %name, "Provider unavailable for streaming, skipping");
                continue;
            }

            let provider_name = self.providers[idx].0.name.clone();
            let (_, provider) = &self.providers[idx];
            let stream = provider.stream(request);

            let failover_warning = self.build_failover_warning(&provider_name);
            if let Some(ref warning) = failover_warning {
                tracing::warn!(%warning, "Failover occurred (streaming)");
            }

            return Ok(StreamSelection {
                stream,
                provider_name,
                failover_warning,
            });
        }

        Err(LlmError::Provider {
            message: "All providers in fallback chain are unavailable. Run `bnity provider status` for details.".to_string(),
        })
    }

    /// Record a stream success for the named provider.
    ///
    /// Call after stream completes without error to update health tracking.
    pub fn record_stream_success(&mut self, provider_name: &str) {
        if let Some((health, _)) = self
            .providers
            .iter_mut()
            .find(|(h, _)| h.name == provider_name)
        {
            health.record_success();
        }
    }

    /// Record a stream failure for the named provider.
    ///
    /// Call after stream emits an error to update health tracking.
    pub fn record_stream_failure(&mut self, provider_name: &str, error: &LlmError) {
        if let Some((health, _)) = self
            .providers
            .iter_mut()
            .find(|(h, _)| h.name == provider_name)
        {
            health.record_failure(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::llm::{ProviderCapabilities, StopReason, Usage};
    use crate::llm::provider::LlmProvider;
    use futures_util::StreamExt;
    use std::future::Future;

    // --- Mock providers ---

    struct MockProvider {
        name: String,
        capabilities: ProviderCapabilities,
        result: MockResult,
    }

    #[derive(Clone)]
    enum MockResult {
        Success(CompletionResponse),
        Error(MockError),
    }

    #[derive(Clone)]
    enum MockError {
        Provider(String),
        Auth,
        RateLimited(Option<u64>),
    }

    impl MockProvider {
        fn ok(name: &str, caps: ProviderCapabilities) -> Self {
            Self {
                name: name.to_string(),
                capabilities: caps,
                result: MockResult::Success(CompletionResponse {
                    id: format!("resp-{name}"),
                    content: format!("Hello from {name}"),
                    model: format!("{name}-model"),
                    stop_reason: StopReason::EndTurn,
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 20,
                        ..Default::default()
                    },
                }),
            }
        }

        fn failing(name: &str, caps: ProviderCapabilities, error: MockError) -> Self {
            Self {
                name: name.to_string(),
                capabilities: caps,
                result: MockResult::Error(error),
            }
        }
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            &self.capabilities
        }

        fn complete(
            &self,
            _request: &CompletionRequest,
        ) -> impl Future<Output = Result<CompletionResponse, LlmError>> + Send {
            let result = self.result.clone();
            async move {
                match result {
                    MockResult::Success(resp) => Ok(resp),
                    MockResult::Error(err) => Err(match err {
                        MockError::Provider(msg) => LlmError::Provider { message: msg },
                        MockError::Auth => LlmError::AuthenticationFailed,
                        MockError::RateLimited(retry_after) => {
                            LlmError::RateLimited {
                                retry_after_ms: retry_after,
                            }
                        }
                    }),
                }
            }
        }

        fn stream(
            &self,
            _request: CompletionRequest,
        ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
            let result = self.result.clone();
            Box::pin(async_stream::stream! {
                match result {
                    MockResult::Success(_) => {
                        yield Ok(StreamEvent::Connected);
                        yield Ok(StreamEvent::Done);
                    }
                    MockResult::Error(err) => {
                        yield Err(match err {
                            MockError::Provider(msg) => LlmError::Provider { message: msg },
                            MockError::Auth => LlmError::AuthenticationFailed,
                            MockError::RateLimited(retry_after) => {
                                LlmError::RateLimited { retry_after_ms: retry_after }
                            }
                        });
                    }
                }
            })
        }

        fn count_tokens(
            &self,
            _request: &CompletionRequest,
        ) -> impl Future<Output = Result<boternity_types::llm::TokenCount, LlmError>> + Send {
            async { Ok(boternity_types::llm::TokenCount { input_tokens: 10 }) }
        }
    }

    fn default_caps() -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false,
            extended_thinking: false,
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
        }
    }

    fn small_caps() -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: false,
            vision: false,
            extended_thinking: false,
            max_context_tokens: 32_000,
            max_output_tokens: 4_096,
        }
    }

    fn make_config(names: &[(&str, u32)]) -> FallbackChainConfig {
        FallbackChainConfig {
            providers: names
                .iter()
                .map(|(name, priority)| boternity_types::llm::ProviderConfig {
                    name: name.to_string(),
                    provider_type: boternity_types::llm::ProviderType::Anthropic,
                    api_key_secret_name: None,
                    base_url: None,
                    model: format!("{name}-model"),
                    priority: *priority,
                    enabled: true,
                    capabilities: default_caps(),
                })
                .collect(),
            rate_limit_queue_timeout_ms: 5000,
            cost_warning_multiplier: 3.0,
        }
    }

    fn test_request() -> CompletionRequest {
        CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![],
            system: None,
            max_tokens: 100,
            temperature: None,
            stream: false,
            stop_sequences: None,
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_happy_path_primary_succeeds() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::ok("primary", default_caps())),
            BoxLlmProvider::new(MockProvider::ok("secondary", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let result = chain.complete(&test_request()).await.unwrap();

        assert_eq!(result.provider_name, "primary");
        assert!(result.failover_warning.is_none());
        assert_eq!(result.response.content, "Hello from primary");
    }

    #[tokio::test]
    async fn test_failover_primary_down_secondary_succeeds() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::failing(
                "primary",
                default_caps(),
                MockError::Provider("500 Internal Server Error".to_string()),
            )),
            BoxLlmProvider::new(MockProvider::ok("secondary", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let result = chain.complete(&test_request()).await.unwrap();

        assert_eq!(result.provider_name, "secondary");
        assert!(result.failover_warning.is_some());
        assert!(result
            .failover_warning
            .unwrap()
            .contains("Switched to secondary"));
    }

    #[tokio::test]
    async fn test_no_failover_on_auth_error() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::failing(
                "primary",
                default_caps(),
                MockError::Auth,
            )),
            BoxLlmProvider::new(MockProvider::ok("secondary", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let result = chain.complete(&test_request()).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LlmError::AuthenticationFailed
        ));
    }

    #[tokio::test]
    async fn test_all_providers_down() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::failing(
                "primary",
                default_caps(),
                MockError::Provider("timeout".to_string()),
            )),
            BoxLlmProvider::new(MockProvider::failing(
                "secondary",
                default_caps(),
                MockError::Provider("timeout".to_string()),
            )),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let result = chain.complete(&test_request()).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("timeout"),
            "Expected last provider's error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_all_providers_unavailable_gives_clear_message() {
        let config = make_config(&[("primary", 0)]);
        let providers = vec![BoxLlmProvider::new(MockProvider::ok(
            "primary",
            default_caps(),
        ))];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());

        // Open the circuit breaker manually
        let error = LlmError::Provider {
            message: "down".to_string(),
        };
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);

        let result = chain.complete(&test_request()).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("bnity provider status"),
            "Expected user-friendly message with CLI hint, got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_cost_warning_when_fallback_expensive() {
        let config = make_config(&[("cheap", 0), ("expensive", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::failing(
                "cheap",
                default_caps(),
                MockError::Provider("down".to_string()),
            )),
            BoxLlmProvider::new(MockProvider::ok("expensive", default_caps())),
        ];

        let mut cost_table = HashMap::new();
        cost_table.insert(
            "cheap".to_string(),
            ProviderCostInfo {
                provider_name: "cheap".to_string(),
                model: "cheap-model".to_string(),
                input_cost_per_million: 1.0,
                output_cost_per_million: 3.0,
            },
        );
        cost_table.insert(
            "expensive".to_string(),
            ProviderCostInfo {
                provider_name: "expensive".to_string(),
                model: "expensive-model".to_string(),
                input_cost_per_million: 10.0,
                output_cost_per_million: 30.0,
            },
        );

        let mut chain = FallbackChain::new(config, providers, cost_table);
        let result = chain.complete(&test_request()).await.unwrap();

        assert_eq!(result.provider_name, "expensive");
        let warning = result.failover_warning.unwrap();
        assert!(
            warning.contains("costs ~"),
            "Expected cost warning, got: {warning}"
        );
        assert!(
            warning.contains("more than cheap"),
            "Expected primary name in cost warning, got: {warning}"
        );
    }

    #[tokio::test]
    async fn test_capability_downgrade_warning() {
        let config = make_config(&[("strong", 0), ("weak", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::failing(
                "strong",
                default_caps(),
                MockError::Provider("down".to_string()),
            )),
            BoxLlmProvider::new(MockProvider::ok("weak", small_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let result = chain.complete(&test_request()).await.unwrap();

        let warning = result.failover_warning.unwrap();
        assert!(
            warning.contains("smaller model"),
            "Expected capability downgrade warning, got: {warning}"
        );
    }

    #[tokio::test]
    async fn test_primary_available() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::ok("primary", default_caps())),
            BoxLlmProvider::new(MockProvider::ok("secondary", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        assert!(chain.primary_available());

        // Break the primary
        let error = LlmError::Provider {
            message: "down".to_string(),
        };
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);
        assert!(!chain.primary_available());
    }

    #[tokio::test]
    async fn test_health_status_returns_all_providers() {
        let config = make_config(&[("a", 0), ("b", 1), ("c", 2)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::ok("a", default_caps())),
            BoxLlmProvider::new(MockProvider::ok("b", default_caps())),
            BoxLlmProvider::new(MockProvider::ok("c", default_caps())),
        ];

        let chain = FallbackChain::new(config, providers, HashMap::new());
        let status = chain.health_status();
        assert_eq!(status.len(), 3);
        assert_eq!(status[0].name, "a");
        assert_eq!(status[1].name, "b");
        assert_eq!(status[2].name, "c");
    }

    #[tokio::test]
    async fn test_select_stream_happy_path() {
        let config = make_config(&[("primary", 0)]);
        let providers = vec![BoxLlmProvider::new(MockProvider::ok(
            "primary",
            default_caps(),
        ))];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let selection = chain.select_stream(test_request()).unwrap();

        assert_eq!(selection.provider_name, "primary");
        assert!(selection.failover_warning.is_none());

        // Consume the stream
        let events: Vec<_> = selection.stream.collect().await;
        assert_eq!(events.len(), 2); // Connected + Done
    }

    #[tokio::test]
    async fn test_select_stream_failover() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::ok("primary", default_caps())),
            BoxLlmProvider::new(MockProvider::ok("secondary", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());

        // Break primary circuit breaker
        let error = LlmError::Provider {
            message: "down".to_string(),
        };
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);

        let selection = chain.select_stream(test_request()).unwrap();
        assert_eq!(selection.provider_name, "secondary");
        assert!(selection.failover_warning.is_some());
    }

    #[tokio::test]
    async fn test_select_stream_all_unavailable() {
        let config = make_config(&[("primary", 0)]);
        let providers = vec![BoxLlmProvider::new(MockProvider::ok(
            "primary",
            default_caps(),
        ))];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());

        // Break circuit
        let error = LlmError::Provider {
            message: "down".to_string(),
        };
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);
        chain.providers[0].0.record_failure(&error);

        let result = chain.select_stream(test_request());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("bnity provider status"));
    }

    #[tokio::test]
    async fn test_record_stream_success_updates_health() {
        let config = make_config(&[("primary", 0)]);
        let providers = vec![BoxLlmProvider::new(MockProvider::ok(
            "primary",
            default_caps(),
        ))];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        chain.record_stream_success("primary");

        assert_eq!(chain.providers[0].0.total_calls, 1);
    }

    #[tokio::test]
    async fn test_record_stream_failure_updates_health() {
        let config = make_config(&[("primary", 0)]);
        let providers = vec![BoxLlmProvider::new(MockProvider::ok(
            "primary",
            default_caps(),
        ))];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        let error = LlmError::Stream("broken".to_string());
        chain.record_stream_failure("primary", &error);

        assert_eq!(chain.providers[0].0.total_failures, 1);
    }

    #[tokio::test]
    async fn test_priority_ordering_with_latency_tiebreak() {
        // Two providers at same priority -- should tiebreak by latency
        let config = make_config(&[("slow", 0), ("fast", 0)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::ok("slow", default_caps())),
            BoxLlmProvider::new(MockProvider::ok("fast", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        // Set known latencies
        chain.providers[0].0.last_latency_ms = Some(500);
        chain.providers[1].0.last_latency_ms = Some(100);

        let result = chain.complete(&test_request()).await.unwrap();
        assert_eq!(result.provider_name, "fast");
    }

    #[tokio::test]
    async fn test_rate_limited_sets_timer_and_failover() {
        let config = make_config(&[("primary", 0), ("secondary", 1)]);
        let providers = vec![
            BoxLlmProvider::new(MockProvider::failing(
                "primary",
                default_caps(),
                MockError::RateLimited(Some(60_000)), // 60s retry -- longer than queue timeout
            )),
            BoxLlmProvider::new(MockProvider::ok("secondary", default_caps())),
        ];

        let mut chain = FallbackChain::new(config, providers, HashMap::new());
        // Set short queue timeout so we fail over instead of waiting
        chain.rate_limit_queue_timeout_ms = 100;

        let result = chain.complete(&test_request()).await.unwrap();
        assert_eq!(result.provider_name, "secondary");

        // Primary should be rate-limited
        assert!(chain.providers[0].0.rate_limit_until.is_some());
    }
}
