//! Bot file storage infrastructure.
//!
//! Implements the `FileStore` trait from `boternity-core` for local filesystem
//! storage with version history, plus text chunking and semantic indexing.

pub mod chunker;
pub mod filesystem;
pub mod indexer;
