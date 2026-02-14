//! Builder system for interactive bot/skill creation.
//!
//! Defines the surface-agnostic `BuilderAgent` trait, `BuilderState`
//! accumulator logic, the Forge system prompt builder that drives
//! the LLM conversation, and persistence traits for draft auto-save
//! and builder memory (past session recall).

pub mod agent;
pub mod assembler;
pub mod defaults;
pub mod draft_store;
pub mod memory;
pub mod prompt;
pub mod state;
