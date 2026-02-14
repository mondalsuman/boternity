//! Builder system for interactive bot/skill creation.
//!
//! Defines the surface-agnostic `BuilderAgent` trait, `BuilderState`
//! accumulator logic, and the Forge system prompt builder that drives
//! the LLM conversation.

pub mod agent;
pub mod prompt;
pub mod state;
