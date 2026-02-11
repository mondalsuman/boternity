//! Memory persistence and extraction for Boternity.
//!
//! This module defines the `MemoryRepository` trait that the infrastructure
//! layer implements for long-term memory and pending extraction CRUD,
//! and the `SessionMemoryExtractor` that uses an LLM to identify key
//! facts worth persisting across sessions.

pub mod extractor;
pub mod store;
