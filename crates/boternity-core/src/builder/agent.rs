//! BuilderAgent trait -- the surface-agnostic builder conversation interface.
//!
//! Both CLI and web adapters implement or consume this trait to drive the
//! interactive bot/skill creation flow. Uses RPITIT (no async_trait)
//! consistent with all project traits.

use std::fmt;
use std::future::Future;

use boternity_types::builder::{
    BuilderAnswer, BuilderConfig, BuilderState, BuilderTurn,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during builder agent operations.
#[derive(Debug)]
pub enum BuilderError {
    /// LLM call failed (network, rate limit, etc.).
    LlmError(String),
    /// Could not parse the LLM's structured output into a BuilderTurn.
    ParseError(String),
    /// Invalid state transition (e.g., going back from the first phase).
    StateError(String),
    /// Bot or skill assembly failed after ReadyToAssemble.
    AssemblyError(String),
}

impl fmt::Display for BuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LlmError(msg) => write!(f, "LLM error: {msg}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
            Self::StateError(msg) => write!(f, "state error: {msg}"),
            Self::AssemblyError(msg) => write!(f, "assembly error: {msg}"),
        }
    }
}

impl std::error::Error for BuilderError {}

// ---------------------------------------------------------------------------
// BuilderAgent trait
// ---------------------------------------------------------------------------

/// Surface-agnostic builder conversation interface.
///
/// Implementations drive the multi-turn conversation between the user and
/// Forge (the builder LLM). Both CLI and web adapters consume this trait.
///
/// Uses RPITIT (return position `impl Trait` in traits) consistent with
/// all async traits in this project -- no `async_trait` macro.
pub trait BuilderAgent {
    /// Start a new builder session from an initial description.
    ///
    /// Creates a fresh `BuilderState` and returns the first `BuilderTurn`
    /// (typically an `AskQuestion` about the bot's name or purpose).
    fn start(
        &self,
        session_id: Uuid,
        initial_description: &str,
    ) -> impl Future<Output = Result<BuilderTurn, BuilderError>> + Send;

    /// Process a user answer and return the next BuilderTurn.
    ///
    /// The answer is recorded in the state, the Forge prompt is rebuilt
    /// with accumulated context, and the LLM produces the next turn.
    fn next_turn(
        &self,
        state: &mut BuilderState,
        answer: BuilderAnswer,
    ) -> impl Future<Output = Result<BuilderTurn, BuilderError>> + Send;

    /// Resume a builder session from saved state (draft restoration).
    ///
    /// Rebuilds the Forge prompt from the existing state and produces
    /// the next question based on where the user left off.
    fn resume(
        &self,
        state: &BuilderState,
    ) -> impl Future<Output = Result<BuilderTurn, BuilderError>> + Send;

    /// Reconfigure an existing bot.
    ///
    /// Loads the current `BuilderConfig`, shows it to the user via
    /// a `ShowPreview`, and asks what to adjust.
    fn reconfigure(
        &self,
        state: &mut BuilderState,
        current_config: BuilderConfig,
    ) -> impl Future<Output = Result<BuilderTurn, BuilderError>> + Send;
}
