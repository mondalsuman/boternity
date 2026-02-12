//! Memory persistence and extraction for Boternity.
//!
//! This module defines the `MemoryRepository` trait that the infrastructure
//! layer implements for long-term memory and pending extraction CRUD,
//! the `SessionMemoryExtractor` that uses an LLM to identify key
//! facts worth persisting across sessions, and the `BoxVectorMemoryStore`
//! and `BoxEmbedder` for type-erased dynamic dispatch of RPITIT traits.

pub mod box_embedder;
pub mod box_vector;
pub mod embedder;
pub mod extractor;
pub mod shared;
pub mod store;
pub mod vector;
