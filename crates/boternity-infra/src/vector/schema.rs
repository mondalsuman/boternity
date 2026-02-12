//! Arrow schema definitions for LanceDB vector tables.
//!
//! Defines the schemas for bot memory, shared memory, and file chunks tables.
//! Each schema includes a 384-dimensional float32 vector field for BGESmallENV15 embeddings.
//!
//! Arrow versions MUST match lancedb's transitive dependency (57.3 for lancedb 0.26).

use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema};

/// BGESmallENV15 embedding dimension.
pub const EMBEDDING_DIMENSION: i32 = 384;

/// Schema for per-bot memory tables in LanceDB.
///
/// Each bot has its own table named `bot_memory_{bot_id}`.
/// Stores extracted facts with vector embeddings for semantic search.
pub fn bot_memory_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("bot_id", DataType::Utf8, false),
        Field::new("fact", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("importance", DataType::Int32, false),
        Field::new("session_id", DataType::Utf8, true),
        Field::new("created_at", DataType::Utf8, false),
        Field::new("last_accessed_at", DataType::Utf8, true),
        Field::new("access_count", DataType::Int32, false),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIMENSION,
            ),
            false,
        ),
    ])
}

/// Schema for the shared memory table in LanceDB.
///
/// A single table named `shared_memory` stores cross-bot shared memories
/// with trust-level partitioning and provenance tracking.
pub fn shared_memory_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("fact", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("importance", DataType::Int32, false),
        Field::new("author_bot_id", DataType::Utf8, false),
        Field::new("author_bot_name", DataType::Utf8, false),
        Field::new("trust_level", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
        Field::new("write_hash", DataType::Utf8, false),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIMENSION,
            ),
            false,
        ),
    ])
}

/// Schema for per-bot file chunk tables in LanceDB.
///
/// Each bot has its own table named `file_chunks_{bot_id}`.
/// Stores chunked text content from uploaded files for semantic search.
pub fn file_chunks_schema() -> Schema {
    Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("file_id", DataType::Utf8, false),
        Field::new("bot_id", DataType::Utf8, false),
        Field::new("filename", DataType::Utf8, false),
        Field::new("chunk_index", DataType::Int32, false),
        Field::new("chunk_text", DataType::Utf8, false),
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIMENSION,
            ),
            false,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_memory_schema_has_correct_fields() {
        let schema = bot_memory_schema();
        assert_eq!(schema.fields().len(), 11);
        assert!(schema.field_with_name("id").is_ok());
        assert!(schema.field_with_name("bot_id").is_ok());
        assert!(schema.field_with_name("fact").is_ok());
        assert!(schema.field_with_name("vector").is_ok());

        let vector_field = schema.field_with_name("vector").unwrap();
        match vector_field.data_type() {
            DataType::FixedSizeList(_, size) => assert_eq!(*size, EMBEDDING_DIMENSION),
            other => panic!("Expected FixedSizeList, got {:?}", other),
        }
    }

    #[test]
    fn test_shared_memory_schema_has_correct_fields() {
        let schema = shared_memory_schema();
        assert_eq!(schema.fields().len(), 11);
        assert!(schema.field_with_name("author_bot_id").is_ok());
        assert!(schema.field_with_name("trust_level").is_ok());
        assert!(schema.field_with_name("write_hash").is_ok());
        assert!(schema.field_with_name("vector").is_ok());
    }

    #[test]
    fn test_file_chunks_schema_has_correct_fields() {
        let schema = file_chunks_schema();
        assert_eq!(schema.fields().len(), 8);
        assert!(schema.field_with_name("chunk_id").is_ok());
        assert!(schema.field_with_name("file_id").is_ok());
        assert!(schema.field_with_name("chunk_text").is_ok());
        assert!(schema.field_with_name("vector").is_ok());
    }

    #[test]
    fn test_embedding_dimension_constant() {
        assert_eq!(EMBEDDING_DIMENSION, 384);
    }
}
