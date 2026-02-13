//! Event types for the Boternity agent event bus.
//!
//! `AgentEvent` is the unified event type broadcast during agent execution.
//! All variants are Clone + Send + Sync for use with tokio broadcast channels.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted during agent hierarchy execution.
///
/// Used by the event bus to communicate agent lifecycle, budget,
/// and safety events to subscribers (UI, logging, orchestrator).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A new agent has been spawned.
    AgentSpawned {
        agent_id: Uuid,
        parent_id: Option<Uuid>,
        task_description: String,
        depth: u8,
        /// Index of this agent among its siblings (0-based).
        index: usize,
        /// Total number of sibling agents in this spawn batch.
        total: usize,
    },

    /// A streaming text delta from a sub-agent.
    AgentTextDelta { agent_id: Uuid, text: String },

    /// A sub-agent has completed successfully.
    AgentCompleted {
        agent_id: Uuid,
        result_summary: String,
        tokens_used: u32,
        duration_ms: u64,
    },

    /// A sub-agent has failed.
    AgentFailed {
        agent_id: Uuid,
        error: String,
        will_retry: bool,
    },

    /// A sub-agent has been cancelled.
    AgentCancelled { agent_id: Uuid, reason: String },

    /// Periodic budget update during execution.
    BudgetUpdate {
        request_id: Uuid,
        tokens_used: u32,
        budget_total: u32,
        percentage: f32,
    },

    /// Budget has crossed the 80% warning threshold.
    BudgetWarning {
        request_id: Uuid,
        tokens_used: u32,
        budget_total: u32,
    },

    /// Budget has been exhausted.
    BudgetExhausted {
        request_id: Uuid,
        tokens_used: u32,
        budget_total: u32,
        completed_agents: Vec<Uuid>,
        incomplete_agents: Vec<Uuid>,
    },

    /// A spawn was rejected because it would exceed the max depth.
    DepthLimitReached {
        agent_id: Uuid,
        attempted_depth: u8,
        max_depth: u8,
    },

    /// A potential infinite loop was detected in the agent hierarchy.
    CycleDetected {
        agent_id: Uuid,
        cycle_description: String,
    },

    /// The root agent has started synthesizing a final response.
    SynthesisStarted { request_id: Uuid },

    /// A sub-agent created a new memory entry.
    MemoryCreated { agent_id: Uuid, fact: String },

    /// An LLM provider failover occurred.
    ProviderFailover {
        from_provider: String,
        to_provider: String,
        reason: String,
    },
}

impl AgentEvent {
    /// Returns the agent_id from variants that carry one, or None for
    /// request-scoped and provider-scoped events.
    pub fn agent_id(&self) -> Option<Uuid> {
        match self {
            AgentEvent::AgentSpawned { agent_id, .. }
            | AgentEvent::AgentTextDelta { agent_id, .. }
            | AgentEvent::AgentCompleted { agent_id, .. }
            | AgentEvent::AgentFailed { agent_id, .. }
            | AgentEvent::AgentCancelled { agent_id, .. }
            | AgentEvent::DepthLimitReached { agent_id, .. }
            | AgentEvent::CycleDetected { agent_id, .. }
            | AgentEvent::MemoryCreated { agent_id, .. } => Some(*agent_id),

            AgentEvent::BudgetUpdate { .. }
            | AgentEvent::BudgetWarning { .. }
            | AgentEvent::BudgetExhausted { .. }
            | AgentEvent::SynthesisStarted { .. }
            | AgentEvent::ProviderFailover { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_uuid() -> Uuid {
        Uuid::now_v7()
    }

    #[test]
    fn test_agent_spawned_serde_roundtrip() {
        let event = AgentEvent::AgentSpawned {
            agent_id: sample_uuid(),
            parent_id: Some(sample_uuid()),
            task_description: "Research topic".to_string(),
            depth: 1,
            index: 0,
            total: 3,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"agent_spawned\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::AgentSpawned { depth: 1, .. }));
    }

    #[test]
    fn test_agent_text_delta_serde_roundtrip() {
        let event = AgentEvent::AgentTextDelta {
            agent_id: sample_uuid(),
            text: "Hello world".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"agent_text_delta\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::AgentTextDelta { .. }));
    }

    #[test]
    fn test_agent_completed_serde_roundtrip() {
        let event = AgentEvent::AgentCompleted {
            agent_id: sample_uuid(),
            result_summary: "Done".to_string(),
            tokens_used: 1500,
            duration_ms: 3200,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"agent_completed\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            AgentEvent::AgentCompleted {
                tokens_used: 1500,
                ..
            }
        ));
    }

    #[test]
    fn test_agent_failed_serde_roundtrip() {
        let event = AgentEvent::AgentFailed {
            agent_id: sample_uuid(),
            error: "timeout".to_string(),
            will_retry: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"agent_failed\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            AgentEvent::AgentFailed {
                will_retry: true,
                ..
            }
        ));
    }

    #[test]
    fn test_agent_cancelled_serde_roundtrip() {
        let event = AgentEvent::AgentCancelled {
            agent_id: sample_uuid(),
            reason: "user requested".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"agent_cancelled\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::AgentCancelled { .. }));
    }

    #[test]
    fn test_budget_update_serde_roundtrip() {
        let event = AgentEvent::BudgetUpdate {
            request_id: sample_uuid(),
            tokens_used: 250_000,
            budget_total: 500_000,
            percentage: 50.0,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"budget_update\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            AgentEvent::BudgetUpdate {
                tokens_used: 250_000,
                ..
            }
        ));
    }

    #[test]
    fn test_budget_warning_serde_roundtrip() {
        let event = AgentEvent::BudgetWarning {
            request_id: sample_uuid(),
            tokens_used: 400_000,
            budget_total: 500_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"budget_warning\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::BudgetWarning { .. }));
    }

    #[test]
    fn test_budget_exhausted_serde_roundtrip() {
        let completed = vec![sample_uuid()];
        let incomplete = vec![sample_uuid(), sample_uuid()];
        let event = AgentEvent::BudgetExhausted {
            request_id: sample_uuid(),
            tokens_used: 510_000,
            budget_total: 500_000,
            completed_agents: completed,
            incomplete_agents: incomplete,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"budget_exhausted\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        if let AgentEvent::BudgetExhausted {
            completed_agents,
            incomplete_agents,
            ..
        } = parsed
        {
            assert_eq!(completed_agents.len(), 1);
            assert_eq!(incomplete_agents.len(), 2);
        } else {
            panic!("expected BudgetExhausted");
        }
    }

    #[test]
    fn test_depth_limit_reached_serde_roundtrip() {
        let event = AgentEvent::DepthLimitReached {
            agent_id: sample_uuid(),
            attempted_depth: 4,
            max_depth: 3,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"depth_limit_reached\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            AgentEvent::DepthLimitReached {
                attempted_depth: 4,
                max_depth: 3,
                ..
            }
        ));
    }

    #[test]
    fn test_cycle_detected_serde_roundtrip() {
        let event = AgentEvent::CycleDetected {
            agent_id: sample_uuid(),
            cycle_description: "A -> B -> A".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"cycle_detected\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::CycleDetected { .. }));
    }

    #[test]
    fn test_synthesis_started_serde_roundtrip() {
        let event = AgentEvent::SynthesisStarted {
            request_id: sample_uuid(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"synthesis_started\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::SynthesisStarted { .. }));
    }

    #[test]
    fn test_memory_created_serde_roundtrip() {
        let event = AgentEvent::MemoryCreated {
            agent_id: sample_uuid(),
            fact: "User prefers dark mode".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"memory_created\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::MemoryCreated { .. }));
    }

    #[test]
    fn test_provider_failover_serde_roundtrip() {
        let event = AgentEvent::ProviderFailover {
            from_provider: "anthropic".to_string(),
            to_provider: "openai".to_string(),
            reason: "rate limited".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"provider_failover\""));
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AgentEvent::ProviderFailover { .. }));
    }

    #[test]
    fn test_agent_id_returns_some_for_agent_scoped_events() {
        let id = sample_uuid();
        let events_with_id = vec![
            AgentEvent::AgentSpawned {
                agent_id: id,
                parent_id: None,
                task_description: "t".to_string(),
                depth: 0,
                index: 0,
                total: 1,
            },
            AgentEvent::AgentTextDelta {
                agent_id: id,
                text: "x".to_string(),
            },
            AgentEvent::AgentCompleted {
                agent_id: id,
                result_summary: "ok".to_string(),
                tokens_used: 0,
                duration_ms: 0,
            },
            AgentEvent::AgentFailed {
                agent_id: id,
                error: "e".to_string(),
                will_retry: false,
            },
            AgentEvent::AgentCancelled {
                agent_id: id,
                reason: "r".to_string(),
            },
            AgentEvent::DepthLimitReached {
                agent_id: id,
                attempted_depth: 4,
                max_depth: 3,
            },
            AgentEvent::CycleDetected {
                agent_id: id,
                cycle_description: "c".to_string(),
            },
            AgentEvent::MemoryCreated {
                agent_id: id,
                fact: "f".to_string(),
            },
        ];
        for event in events_with_id {
            assert_eq!(event.agent_id(), Some(id), "expected Some(id) for {event:?}");
        }
    }

    #[test]
    fn test_agent_id_returns_none_for_non_agent_events() {
        let events_without_id = vec![
            AgentEvent::BudgetUpdate {
                request_id: sample_uuid(),
                tokens_used: 0,
                budget_total: 0,
                percentage: 0.0,
            },
            AgentEvent::BudgetWarning {
                request_id: sample_uuid(),
                tokens_used: 0,
                budget_total: 0,
            },
            AgentEvent::BudgetExhausted {
                request_id: sample_uuid(),
                tokens_used: 0,
                budget_total: 0,
                completed_agents: vec![],
                incomplete_agents: vec![],
            },
            AgentEvent::SynthesisStarted {
                request_id: sample_uuid(),
            },
            AgentEvent::ProviderFailover {
                from_provider: "a".to_string(),
                to_provider: "b".to_string(),
                reason: "r".to_string(),
            },
        ];
        for event in events_without_id {
            assert_eq!(event.agent_id(), None, "expected None for {event:?}");
        }
    }
}
