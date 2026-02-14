//! Builder infrastructure implementations.
//!
//! Contains SQLite-backed implementations of `BuilderDraftStore` and
//! `BuilderMemoryStore` from `boternity-core`, plus the LLM-powered
//! `LlmBuilderAgent` that drives the interactive builder conversation.

pub mod llm_builder;
pub mod sqlite_draft_store;
pub mod sqlite_memory_store;
