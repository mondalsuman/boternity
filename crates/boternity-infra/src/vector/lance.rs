//! LanceDB vector store wrapper for connection management and table operations.
//!
//! Provides `LanceVectorStore` which wraps a `lancedb::Connection` and offers
//! helper methods for table lifecycle (create, open, drop) using Arrow schemas.
