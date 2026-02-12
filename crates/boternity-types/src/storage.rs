//! Storage types for Boternity.
//!
//! These types model bot file storage, file versioning, text chunks
//! for embedding, and key-value store entries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Maximum file size allowed for bot file storage (50 MB).
pub const MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;

/// A file stored in a bot's file storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageFile {
    pub id: Uuid,
    pub bot_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub version: u32,
    /// Whether the file's text content has been chunked and embedded.
    pub is_indexed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A versioned snapshot of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    pub id: Uuid,
    pub file_id: Uuid,
    pub version: u32,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
}

/// A text chunk from a file, prepared for embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub chunk_id: Uuid,
    pub file_id: Uuid,
    pub bot_id: Uuid,
    pub filename: String,
    pub chunk_index: u32,
    pub chunk_text: String,
    pub embedding_model: String,
}

/// A key-value entry in a bot's persistent KV store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvEntry {
    pub bot_id: Uuid,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_file_size() {
        assert_eq!(MAX_FILE_SIZE_BYTES, 52_428_800);
    }

    #[test]
    fn test_storage_file_serialize() {
        let file = StorageFile {
            id: Uuid::now_v7(),
            bot_id: Uuid::now_v7(),
            filename: "notes.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 1024,
            version: 1,
            is_indexed: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("\"filename\":\"notes.txt\""));
        assert!(json.contains("\"is_indexed\":false"));
    }

    #[test]
    fn test_kv_entry_serialize() {
        let entry = KvEntry {
            bot_id: Uuid::now_v7(),
            key: "theme".to_string(),
            value: serde_json::json!("dark"),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"key\":\"theme\""));
        assert!(json.contains("\"dark\""));
    }
}
