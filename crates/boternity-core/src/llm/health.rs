//! Provider health tracking for the fallback chain.
//!
//! Implements a circuit breaker pattern to track provider health and
//! determine when to failover to the next provider. These types live
//! in core (not infra) because `FallbackChain` depends on them.

use std::time::{Duration, Instant};

use boternity_types::llm::{LlmError, ProviderStatusInfo};

/// Circuit breaker state for a provider.
#[derive(Debug, Clone)]
pub enum CircuitState {
    /// Normal operation. Tracks consecutive failures toward threshold.
    Closed {
        consecutive_failures: u32,
    },
    /// Provider is disabled. Will probe after `wait_duration` elapses.
    Open {
        opened_at: Instant,
        wait_duration: Duration,
    },
    /// Probing: one request allowed to test if provider recovered.
    HalfOpen,
}

/// Health tracking for a single LLM provider.
#[derive(Debug)]
pub struct ProviderHealth {
    /// Provider name (matches `ProviderConfig.name`).
    pub name: String,
    /// Priority in fallback ordering (lower = higher priority).
    pub priority: u32,
    /// Current circuit breaker state.
    pub state: CircuitState,
    /// Last error message from this provider.
    pub last_error: Option<String>,
    /// When this provider last succeeded.
    pub last_success: Option<Instant>,
    /// Latency of the last call in milliseconds.
    pub last_latency_ms: Option<u64>,
    /// Total calls routed to this provider.
    pub total_calls: u64,
    /// Total failed calls.
    pub total_failures: u64,
    /// When this provider first became available (for uptime tracking).
    pub uptime_since: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of successes in HalfOpen before closing the circuit.
    pub success_threshold: u32,
    /// How long to wait in Open state before probing.
    pub open_duration: Duration,
    /// If rate-limited, don't use until this instant.
    pub rate_limit_until: Option<Instant>,
}

impl ProviderHealth {
    /// Create a new health tracker with sensible defaults.
    pub fn new(name: impl Into<String>, priority: u32) -> Self {
        Self {
            name: name.into(),
            priority,
            state: CircuitState::Closed {
                consecutive_failures: 0,
            },
            last_error: None,
            last_success: None,
            last_latency_ms: None,
            total_calls: 0,
            total_failures: 0,
            uptime_since: Some(chrono::Utc::now()),
            failure_threshold: 3,
            success_threshold: 1,
            open_duration: Duration::from_secs(30),
            rate_limit_until: None,
        }
    }

    /// Check whether this provider is available for routing.
    ///
    /// Handles rate-limit cooldown and circuit state transitions
    /// (Open -> HalfOpen when the wait duration has elapsed).
    pub fn is_available(&mut self) -> bool {
        // Check rate limit
        if let Some(until) = self.rate_limit_until {
            if Instant::now() < until {
                return false;
            }
            self.rate_limit_until = None;
        }

        match &self.state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open {
                opened_at,
                wait_duration,
            } => {
                if opened_at.elapsed() >= *wait_duration {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call to this provider.
    pub fn record_success(&mut self) {
        self.total_calls += 1;
        self.last_success = Some(Instant::now());

        match &self.state {
            CircuitState::HalfOpen => {
                // Recovery confirmed, close the circuit
                self.state = CircuitState::Closed {
                    consecutive_failures: 0,
                };
                self.uptime_since = Some(chrono::Utc::now());
            }
            CircuitState::Closed { .. } => {
                self.state = CircuitState::Closed {
                    consecutive_failures: 0,
                };
            }
            CircuitState::Open { .. } => {
                // Should not happen (calls shouldn't reach here when open)
                // but handle gracefully
                self.state = CircuitState::Closed {
                    consecutive_failures: 0,
                };
                self.uptime_since = Some(chrono::Utc::now());
            }
        }
    }

    /// Record a failed call to this provider.
    pub fn record_failure(&mut self, error: &LlmError) {
        self.total_calls += 1;
        self.total_failures += 1;
        self.last_error = Some(error.to_string());

        match &self.state {
            CircuitState::Closed {
                consecutive_failures,
            } => {
                let new_count = consecutive_failures + 1;
                if new_count >= self.failure_threshold {
                    self.state = CircuitState::Open {
                        opened_at: Instant::now(),
                        wait_duration: self.open_duration,
                    };
                    self.uptime_since = None;
                } else {
                    self.state = CircuitState::Closed {
                        consecutive_failures: new_count,
                    };
                }
            }
            CircuitState::HalfOpen => {
                // Probe failed, reopen the circuit
                self.state = CircuitState::Open {
                    opened_at: Instant::now(),
                    wait_duration: self.open_duration,
                };
                self.uptime_since = None;
            }
            CircuitState::Open { .. } => {
                // Already open, no state change
            }
        }
    }

    /// Mark this provider as rate-limited.
    ///
    /// Uses the provider's `retry_after_ms` hint if available, capped at `max_wait_ms`.
    pub fn set_rate_limited(&mut self, retry_after_ms: Option<u64>, max_wait_ms: u64) {
        let wait_ms = retry_after_ms.unwrap_or(max_wait_ms).min(max_wait_ms);
        self.rate_limit_until = Some(Instant::now() + Duration::from_millis(wait_ms));
    }

    /// Classify whether an error should trigger failover to the next provider.
    ///
    /// Failover errors (transient/provider-side):
    /// - Provider, Stream, RateLimited, Overloaded
    ///
    /// Non-failover errors (request/auth issues -- won't help to try another provider):
    /// - AuthenticationFailed, InvalidRequest, ContextLengthExceeded
    pub fn is_failover_error(error: &LlmError) -> bool {
        matches!(
            error,
            LlmError::Provider { .. }
                | LlmError::Stream(..)
                | LlmError::RateLimited { .. }
                | LlmError::Overloaded(..)
        )
    }

    /// Convert to a `ProviderStatusInfo` for CLI display.
    pub fn to_status_info(&self) -> ProviderStatusInfo {
        let circuit_state = match &self.state {
            CircuitState::Closed { .. } => "closed".to_string(),
            CircuitState::Open { .. } => "open".to_string(),
            CircuitState::HalfOpen => "half_open".to_string(),
        };

        let last_success_ago = self.last_success.map(|s| {
            let elapsed = s.elapsed();
            if elapsed.as_secs() < 60 {
                format!("{}s ago", elapsed.as_secs())
            } else if elapsed.as_secs() < 3600 {
                format!("{}m ago", elapsed.as_secs() / 60)
            } else {
                format!("{}h ago", elapsed.as_secs() / 3600)
            }
        });

        ProviderStatusInfo {
            name: self.name.clone(),
            circuit_state,
            last_error: self.last_error.clone(),
            last_success_ago,
            total_calls: self.total_calls,
            total_failures: self.total_failures,
            uptime_since: self.uptime_since.map(|t| t.to_rfc3339()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_provider_health_defaults() {
        let health = ProviderHealth::new("anthropic", 0);
        assert_eq!(health.name, "anthropic");
        assert_eq!(health.priority, 0);
        assert_eq!(health.failure_threshold, 3);
        assert_eq!(health.success_threshold, 1);
        assert_eq!(health.open_duration, Duration::from_secs(30));
        assert!(matches!(
            health.state,
            CircuitState::Closed {
                consecutive_failures: 0
            }
        ));
    }

    #[test]
    fn test_is_available_when_closed() {
        let mut health = ProviderHealth::new("test", 0);
        assert!(health.is_available());
    }

    #[test]
    fn test_circuit_opens_after_threshold_failures() {
        let mut health = ProviderHealth::new("test", 0);
        let error = LlmError::Provider {
            message: "timeout".to_string(),
        };

        health.record_failure(&error);
        health.record_failure(&error);
        assert!(health.is_available()); // 2 failures, threshold is 3

        health.record_failure(&error);
        assert!(!health.is_available()); // 3 failures, circuit opens
        assert!(matches!(health.state, CircuitState::Open { .. }));
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut health = ProviderHealth::new("test", 0);
        let error = LlmError::Provider {
            message: "timeout".to_string(),
        };

        health.record_failure(&error);
        health.record_failure(&error);
        health.record_success();

        // Should be back to 0 consecutive failures
        assert!(matches!(
            health.state,
            CircuitState::Closed {
                consecutive_failures: 0
            }
        ));
    }

    #[test]
    fn test_rate_limited_blocks_availability() {
        let mut health = ProviderHealth::new("test", 0);
        health.set_rate_limited(Some(5000), 10000);
        assert!(!health.is_available());
    }

    #[test]
    fn test_is_failover_error_classification() {
        assert!(ProviderHealth::is_failover_error(&LlmError::Provider {
            message: "500".to_string()
        }));
        assert!(ProviderHealth::is_failover_error(&LlmError::Stream(
            "broken pipe".to_string()
        )));
        assert!(ProviderHealth::is_failover_error(&LlmError::RateLimited {
            retry_after_ms: None
        }));
        assert!(ProviderHealth::is_failover_error(&LlmError::Overloaded(
            "busy".to_string()
        )));

        assert!(!ProviderHealth::is_failover_error(
            &LlmError::AuthenticationFailed
        ));
        assert!(!ProviderHealth::is_failover_error(
            &LlmError::InvalidRequest("bad".to_string())
        ));
        assert!(!ProviderHealth::is_failover_error(
            &LlmError::ContextLengthExceeded {
                max: 100000,
                requested: 120000
            }
        ));
    }

    #[test]
    fn test_to_status_info() {
        let health = ProviderHealth::new("anthropic", 0);
        let info = health.to_status_info();
        assert_eq!(info.name, "anthropic");
        assert_eq!(info.circuit_state, "closed");
        assert_eq!(info.total_calls, 0);
        assert!(info.uptime_since.is_some());
    }

    #[test]
    fn test_open_to_half_open_after_timeout() {
        let mut health = ProviderHealth::new("test", 0);
        // Use a very short open_duration so elapsed() >= wait_duration immediately
        health.open_duration = Duration::from_millis(0);

        let error = LlmError::Provider {
            message: "timeout".to_string(),
        };

        // Drive to Open state
        health.record_failure(&error);
        health.record_failure(&error);
        health.record_failure(&error);
        assert!(matches!(health.state, CircuitState::Open { .. }));

        // After wait_duration elapses, is_available transitions to HalfOpen
        assert!(health.is_available());
        assert!(matches!(health.state, CircuitState::HalfOpen));
    }

    #[test]
    fn test_half_open_to_closed_on_success() {
        let mut health = ProviderHealth::new("test", 0);
        health.open_duration = Duration::from_millis(0);

        let error = LlmError::Provider {
            message: "down".to_string(),
        };

        // Drive to Open, then HalfOpen
        health.record_failure(&error);
        health.record_failure(&error);
        health.record_failure(&error);
        assert!(health.is_available()); // transitions Open -> HalfOpen
        assert!(matches!(health.state, CircuitState::HalfOpen));

        // Success in HalfOpen closes the circuit
        health.record_success();
        assert!(matches!(
            health.state,
            CircuitState::Closed {
                consecutive_failures: 0
            }
        ));
        assert!(health.uptime_since.is_some());
    }

    #[test]
    fn test_half_open_to_open_on_failure() {
        let mut health = ProviderHealth::new("test", 0);
        health.open_duration = Duration::from_millis(0);

        let error = LlmError::Provider {
            message: "down".to_string(),
        };

        // Drive to Open, then HalfOpen
        health.record_failure(&error);
        health.record_failure(&error);
        health.record_failure(&error);
        assert!(health.is_available()); // transitions Open -> HalfOpen
        assert!(matches!(health.state, CircuitState::HalfOpen));

        // Failure in HalfOpen reopens the circuit
        health.record_failure(&error);
        assert!(matches!(health.state, CircuitState::Open { .. }));
        assert!(health.uptime_since.is_none());
    }

    #[test]
    fn test_rate_limit_expiry_restores_availability() {
        let mut health = ProviderHealth::new("test", 0);
        // Set rate limit that expires immediately (0ms)
        health.set_rate_limited(Some(0), 5000);

        // Should be available because 0ms has elapsed
        assert!(health.is_available());
        assert!(health.rate_limit_until.is_none()); // cleared after expiry check
    }

    #[test]
    fn test_set_rate_limited_caps_at_max_wait() {
        let mut health = ProviderHealth::new("test", 0);
        // retry_after suggests 10000ms but max_wait is 2000ms
        health.set_rate_limited(Some(10_000), 2_000);

        // The rate_limit_until should be set (capped at max_wait)
        assert!(health.rate_limit_until.is_some());
        assert!(!health.is_available());
    }

    #[test]
    fn test_set_rate_limited_uses_default_when_no_retry_after() {
        let mut health = ProviderHealth::new("test", 0);
        // No retry_after hint, falls back to max_wait_ms
        health.set_rate_limited(None, 5_000);
        assert!(health.rate_limit_until.is_some());
    }

    #[test]
    fn test_record_failure_tracks_metrics() {
        let mut health = ProviderHealth::new("test", 0);
        let error = LlmError::Stream("broken pipe".to_string());

        health.record_failure(&error);
        assert_eq!(health.total_calls, 1);
        assert_eq!(health.total_failures, 1);
        assert_eq!(health.last_error.as_deref(), Some("stream error: broken pipe"));
    }

    #[test]
    fn test_record_success_tracks_metrics() {
        let mut health = ProviderHealth::new("test", 0);
        health.record_success();
        assert_eq!(health.total_calls, 1);
        assert_eq!(health.total_failures, 0);
        assert!(health.last_success.is_some());
    }

    #[test]
    fn test_to_status_info_open_state() {
        let mut health = ProviderHealth::new("test", 0);
        let error = LlmError::Provider {
            message: "500".to_string(),
        };
        health.record_failure(&error);
        health.record_failure(&error);
        health.record_failure(&error);

        let info = health.to_status_info();
        assert_eq!(info.circuit_state, "open");
        assert!(info.last_error.is_some());
        assert_eq!(info.total_failures, 3);
        assert!(info.uptime_since.is_none());
    }

    #[test]
    fn test_to_status_info_half_open_state() {
        let mut health = ProviderHealth::new("test", 0);
        health.open_duration = Duration::from_millis(0);

        let error = LlmError::Provider {
            message: "500".to_string(),
        };
        health.record_failure(&error);
        health.record_failure(&error);
        health.record_failure(&error);
        health.is_available(); // transitions to HalfOpen

        let info = health.to_status_info();
        assert_eq!(info.circuit_state, "half_open");
    }
}
