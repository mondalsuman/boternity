//! BuilderState accumulator logic.
//!
//! The `BuilderState` struct lives in `boternity-types`; this module provides
//! an extension trait (`BuilderStateExt`) with lifecycle management methods:
//! creating new sessions, recording exchanges, navigating phases, and tracking
//! completeness. The extension trait pattern is used because Rust does not
//! allow inherent impls for types defined in another crate.

use boternity_types::builder::{
    BuilderExchange, BuilderPhase, BuilderState, PartialBuilderConfig,
};
use uuid::Uuid;

/// Create a new `BuilderState` for a fresh builder session.
///
/// Starts in the `Basics` phase with an empty conversation and config.
pub fn new_builder_state(session_id: Uuid, initial_description: String) -> BuilderState {
    BuilderState {
        session_id,
        phase: BuilderPhase::Basics,
        initial_description,
        purpose_category: None,
        conversation: Vec::new(),
        config: PartialBuilderConfig::default(),
        phase_history: Vec::new(),
    }
}

/// Extension trait for `BuilderState` lifecycle management.
///
/// Provides conversation accumulation, phase navigation (including back
/// navigation with truncation), config updates, and completeness checks.
pub trait BuilderStateExt {
    /// Record a question-answer exchange in the conversation log.
    fn record_exchange(&mut self, question: String, answer: String);

    /// Advance to a new phase, recording the current phase in history.
    fn advance_phase(&mut self, new_phase: BuilderPhase);

    /// Go back to the previous phase, truncating conversation entries
    /// from the current phase. Returns the phase we went back to, or
    /// `None` if there is no history.
    fn go_back(&mut self) -> Option<BuilderPhase>;

    /// Update a single field in the partial config by name.
    fn update_config_field(&mut self, field: &str, value: serde_json::Value);

    /// Format the last 5 exchanges as a conversation summary.
    fn conversation_summary(&self) -> String;

    /// Total number of question-answer exchanges so far.
    fn question_count(&self) -> usize;

    /// Whether the builder session is complete and ready for assembly.
    fn is_complete(&self) -> bool;
}

impl BuilderStateExt for BuilderState {
    fn record_exchange(&mut self, question: String, answer: String) {
        self.conversation.push(BuilderExchange {
            question,
            answer,
            phase: self.phase.clone(),
        });
    }

    fn advance_phase(&mut self, new_phase: BuilderPhase) {
        self.phase_history.push(self.phase.clone());
        self.phase = new_phase;
    }

    fn go_back(&mut self) -> Option<BuilderPhase> {
        let previous = self.phase_history.pop()?;
        let current_phase = self.phase.clone();

        // Remove all conversation entries from the phase we're leaving
        self.conversation
            .retain(|exchange| exchange.phase != current_phase);

        self.phase = previous.clone();
        Some(previous)
    }

    fn update_config_field(&mut self, field: &str, value: serde_json::Value) {
        match field {
            "name" => {
                self.config.name = serde_json::from_value(value).ok();
            }
            "description" => {
                self.config.description = serde_json::from_value(value).ok();
            }
            "category" => {
                self.config.category = serde_json::from_value(value).ok();
            }
            "tags" => {
                self.config.tags = serde_json::from_value(value).ok();
            }
            "tone" => {
                self.config.tone = serde_json::from_value(value).ok();
            }
            "traits" => {
                self.config.traits = serde_json::from_value(value).ok();
            }
            "purpose" => {
                self.config.purpose = serde_json::from_value(value).ok();
            }
            "boundaries" => {
                self.config.boundaries = serde_json::from_value(value).ok();
            }
            "model" => {
                self.config.model = serde_json::from_value(value).ok();
            }
            "temperature" => {
                self.config.temperature = serde_json::from_value(value).ok();
            }
            "max_tokens" => {
                self.config.max_tokens = serde_json::from_value(value).ok();
            }
            "skills" => {
                if let Ok(skills) = serde_json::from_value(value) {
                    self.config.skills = skills;
                }
            }
            _ => {} // Unknown fields silently ignored
        }
    }

    fn conversation_summary(&self) -> String {
        let recent: Vec<&BuilderExchange> = self
            .conversation
            .iter()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if recent.is_empty() {
            return "No exchanges yet.".to_string();
        }

        recent
            .iter()
            .map(|ex| format!("Q: {}\nA: {}", ex.question, ex.answer))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn question_count(&self) -> usize {
        self.conversation.len()
    }

    fn is_complete(&self) -> bool {
        self.phase == BuilderPhase::Review
            && self.config.name.is_some()
            && self.config.purpose.is_some()
            && self.config.model.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_basics_phase() {
        let state = new_builder_state(Uuid::now_v7(), "A coding bot".to_string());

        assert_eq!(state.phase, BuilderPhase::Basics);
        assert_eq!(state.initial_description, "A coding bot");
        assert!(state.conversation.is_empty());
        assert!(state.phase_history.is_empty());
        assert!(state.config.name.is_none());
        assert!(state.purpose_category.is_none());
    }

    #[test]
    fn test_record_exchange_adds_to_conversation() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());

        state.record_exchange(
            "What is your bot's name?".to_string(),
            "CodeHelper".to_string(),
        );

        assert_eq!(state.conversation.len(), 1);
        assert_eq!(state.conversation[0].question, "What is your bot's name?");
        assert_eq!(state.conversation[0].answer, "CodeHelper");
        assert_eq!(state.conversation[0].phase, BuilderPhase::Basics);
    }

    #[test]
    fn test_advance_phase_moves_and_records_history() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());
        assert_eq!(state.phase, BuilderPhase::Basics);
        assert!(state.phase_history.is_empty());

        state.advance_phase(BuilderPhase::Personality);

        assert_eq!(state.phase, BuilderPhase::Personality);
        assert_eq!(state.phase_history.len(), 1);
        assert_eq!(state.phase_history[0], BuilderPhase::Basics);

        state.advance_phase(BuilderPhase::Model);

        assert_eq!(state.phase, BuilderPhase::Model);
        assert_eq!(state.phase_history.len(), 2);
        assert_eq!(state.phase_history[1], BuilderPhase::Personality);
    }

    #[test]
    fn test_go_back_truncates_correctly() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());

        // Record exchange in Basics
        state.record_exchange("Name?".to_string(), "Luna".to_string());

        // Advance to Personality and record exchange
        state.advance_phase(BuilderPhase::Personality);
        state.record_exchange("Tone?".to_string(), "Friendly".to_string());

        // Advance to Model and record exchange
        state.advance_phase(BuilderPhase::Model);
        state.record_exchange("Model?".to_string(), "Claude".to_string());

        assert_eq!(state.conversation.len(), 3);

        // Go back from Model to Personality
        let went_back_to = state.go_back();
        assert_eq!(went_back_to, Some(BuilderPhase::Personality));
        assert_eq!(state.phase, BuilderPhase::Personality);
        // Model exchange was removed
        assert_eq!(state.conversation.len(), 2);
        assert_eq!(state.conversation[0].phase, BuilderPhase::Basics);
        assert_eq!(state.conversation[1].phase, BuilderPhase::Personality);

        // Go back from Personality to Basics
        let went_back_to = state.go_back();
        assert_eq!(went_back_to, Some(BuilderPhase::Basics));
        assert_eq!(state.phase, BuilderPhase::Basics);
        // Personality exchange was removed
        assert_eq!(state.conversation.len(), 1);
        assert_eq!(state.conversation[0].phase, BuilderPhase::Basics);
    }

    #[test]
    fn test_go_back_from_first_phase_returns_none() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());

        let result = state.go_back();
        assert!(result.is_none());
        assert_eq!(state.phase, BuilderPhase::Basics);
    }

    #[test]
    fn test_conversation_summary_limits_to_last_5() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());

        for i in 0..8 {
            state.record_exchange(format!("Question {i}"), format!("Answer {i}"));
        }

        let summary = state.conversation_summary();

        // Should contain the last 5 exchanges (3..8)
        assert!(!summary.contains("Question 0"));
        assert!(!summary.contains("Question 1"));
        assert!(!summary.contains("Question 2"));
        assert!(summary.contains("Question 3"));
        assert!(summary.contains("Question 4"));
        assert!(summary.contains("Question 5"));
        assert!(summary.contains("Question 6"));
        assert!(summary.contains("Question 7"));
    }

    #[test]
    fn test_conversation_summary_empty_state() {
        let state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());
        let summary = state.conversation_summary();
        assert_eq!(summary, "No exchanges yet.");
    }

    #[test]
    fn test_question_count() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());
        assert_eq!(state.question_count(), 0);

        state.record_exchange("Q1".to_string(), "A1".to_string());
        state.record_exchange("Q2".to_string(), "A2".to_string());
        assert_eq!(state.question_count(), 2);
    }

    #[test]
    fn test_is_complete_requires_review_and_config() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());

        // Not complete in Basics phase
        assert!(!state.is_complete());

        // Not complete in Review without config
        state.phase = BuilderPhase::Review;
        assert!(!state.is_complete());

        // Still not complete with partial config
        state.config.name = Some("TestBot".to_string());
        assert!(!state.is_complete());

        // Complete with all required fields
        state.config.purpose = Some("Help with coding".to_string());
        state.config.model = Some("claude-sonnet-4-20250514".to_string());
        assert!(state.is_complete());
    }

    #[test]
    fn test_update_config_field() {
        let mut state = new_builder_state(Uuid::now_v7(), "Test bot".to_string());

        state.update_config_field("name", serde_json::json!("CodeHelper"));
        assert_eq!(state.config.name, Some("CodeHelper".to_string()));

        state.update_config_field("temperature", serde_json::json!(0.8));
        assert_eq!(state.config.temperature, Some(0.8));

        state.update_config_field(
            "tags",
            serde_json::json!(["coding", "assistant"]),
        );
        assert_eq!(
            state.config.tags,
            Some(vec!["coding".to_string(), "assistant".to_string()])
        );

        // Unknown field is silently ignored
        state.update_config_field("nonexistent", serde_json::json!("value"));
        // No panic, no effect
    }
}
