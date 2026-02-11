//! Session manager for chat sessions.
//!
//! Wraps a `ChatSession` with turn tracking and lifecycle management.
//! Tracks when memory extraction should run (every N turns).

use boternity_types::chat::{ChatSession, SessionStatus};
use chrono::Utc;

/// Default number of turns between memory extraction attempts.
const MEMORY_EXTRACTION_INTERVAL: u32 = 10;

/// Manages the lifecycle and state of a single chat session.
///
/// Wraps a `ChatSession` and adds turn-tracking logic for memory
/// extraction scheduling.
pub struct SessionManager {
    session: ChatSession,
    /// Turn counter (incremented on each user+assistant exchange).
    turn_count: u32,
}

impl SessionManager {
    /// Create a new session manager wrapping an existing session.
    pub fn new(session: ChatSession) -> Self {
        Self {
            session,
            turn_count: 0,
        }
    }

    /// Access the underlying chat session.
    pub fn session(&self) -> &ChatSession {
        &self.session
    }

    /// Get a mutable reference to the underlying chat session.
    pub fn session_mut(&mut self) -> &mut ChatSession {
        &mut self.session
    }

    /// Current turn count within this session.
    pub fn turn_count(&self) -> u32 {
        self.turn_count
    }

    /// Increment the turn counter.
    ///
    /// A "turn" is one user message + one assistant response.
    /// Call this after each complete exchange.
    pub fn increment_turn(&mut self) {
        self.turn_count += 1;
    }

    /// Whether memory extraction should run based on the turn counter.
    ///
    /// Returns true every `MEMORY_EXTRACTION_INTERVAL` turns (default: 10),
    /// ensuring periodic memory capture during long conversations.
    pub fn should_extract_memory(&self) -> bool {
        self.turn_count > 0 && self.turn_count % MEMORY_EXTRACTION_INTERVAL == 0
    }

    /// Mark the session as completed.
    ///
    /// Sets status to `Completed` and records the end timestamp.
    pub fn mark_completed(&mut self) {
        self.session.status = SessionStatus::Completed;
        self.session.ended_at = Some(Utc::now());
    }

    /// Mark the session as crashed.
    ///
    /// Sets status to `Crashed` and records the end timestamp.
    pub fn mark_crashed(&mut self) {
        self.session.status = SessionStatus::Crashed;
        self.session.ended_at = Some(Utc::now());
    }

    /// Update token usage on the session after an LLM response.
    pub fn add_token_usage(&mut self, input_tokens: u32, output_tokens: u32) {
        self.session.total_input_tokens += input_tokens;
        self.session.total_output_tokens += output_tokens;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_session() -> ChatSession {
        ChatSession {
            id: Uuid::now_v7(),
            bot_id: Uuid::now_v7(),
            title: None,
            started_at: Utc::now(),
            ended_at: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            message_count: 0,
            model: "claude-sonnet-4-20250514".to_string(),
            status: SessionStatus::Active,
        }
    }

    #[test]
    fn test_new_session_manager() {
        let mgr = SessionManager::new(test_session());
        assert_eq!(mgr.turn_count(), 0);
        assert_eq!(mgr.session().status, SessionStatus::Active);
    }

    #[test]
    fn test_increment_turn() {
        let mut mgr = SessionManager::new(test_session());
        mgr.increment_turn();
        assert_eq!(mgr.turn_count(), 1);
        mgr.increment_turn();
        assert_eq!(mgr.turn_count(), 2);
    }

    #[test]
    fn test_should_extract_memory() {
        let mut mgr = SessionManager::new(test_session());

        // Turn 0 -- no extraction
        assert!(!mgr.should_extract_memory());

        // Turns 1-9 -- no extraction
        for _ in 0..9 {
            mgr.increment_turn();
            assert!(!mgr.should_extract_memory());
        }

        // Turn 10 -- extract!
        mgr.increment_turn();
        assert_eq!(mgr.turn_count(), 10);
        assert!(mgr.should_extract_memory());

        // Turn 11 -- no extraction
        mgr.increment_turn();
        assert!(!mgr.should_extract_memory());

        // Turn 20 -- extract again!
        for _ in 0..9 {
            mgr.increment_turn();
        }
        assert_eq!(mgr.turn_count(), 20);
        assert!(mgr.should_extract_memory());
    }

    #[test]
    fn test_mark_completed() {
        let mut mgr = SessionManager::new(test_session());
        assert!(mgr.session().ended_at.is_none());

        mgr.mark_completed();
        assert_eq!(mgr.session().status, SessionStatus::Completed);
        assert!(mgr.session().ended_at.is_some());
    }

    #[test]
    fn test_mark_crashed() {
        let mut mgr = SessionManager::new(test_session());
        mgr.mark_crashed();
        assert_eq!(mgr.session().status, SessionStatus::Crashed);
        assert!(mgr.session().ended_at.is_some());
    }

    #[test]
    fn test_add_token_usage() {
        let mut mgr = SessionManager::new(test_session());
        mgr.add_token_usage(100, 200);
        assert_eq!(mgr.session().total_input_tokens, 100);
        assert_eq!(mgr.session().total_output_tokens, 200);

        mgr.add_token_usage(50, 75);
        assert_eq!(mgr.session().total_input_tokens, 150);
        assert_eq!(mgr.session().total_output_tokens, 275);
    }
}
