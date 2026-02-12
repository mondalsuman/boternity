//! Vector database infrastructure for memory embeddings.
//!
//! Provides LanceDB vector store management and fastembed-based local
//! embedding generation. Arrow schemas define the table structures.

pub mod embedder;
pub mod lance;
pub mod memory;
pub mod schema;
