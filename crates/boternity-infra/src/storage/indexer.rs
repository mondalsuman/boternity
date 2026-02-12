//! File indexer for semantic search.
//!
//! Chunks text files, generates embeddings via `FastEmbedEmbedder`, and stores
//! chunk vectors in LanceDB for semantic search. Non-text files are stored
//! but not indexed.
//!
//! Each bot has its own `file_chunks_{bot_id}` table in LanceDB.

use std::sync::Arc;

use arrow_array::{Array, FixedSizeListArray, Int32Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use boternity_core::memory::embedder::Embedder;
use boternity_types::error::RepositoryError;
use boternity_types::storage::FileChunk;
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use uuid::Uuid;

use crate::vector::lance::LanceVectorStore;
use crate::vector::schema::{file_chunks_schema, EMBEDDING_DIMENSION};

use super::chunker::{chunk_text_file, ChunkResult};

/// File indexer that chunks text and stores embeddings in LanceDB.
///
/// Works with any `Embedder` implementation (FastEmbedEmbedder in production).
pub struct FileIndexer<E: Embedder> {
    vector_store: Arc<LanceVectorStore>,
    embedder: Arc<E>,
}

impl<E: Embedder> FileIndexer<E> {
    /// Create a new file indexer.
    pub fn new(vector_store: Arc<LanceVectorStore>, embedder: Arc<E>) -> Self {
        Self {
            vector_store,
            embedder,
        }
    }

    /// Index a text file: chunk it, embed the chunks, and store in LanceDB.
    ///
    /// Returns the list of `FileChunk` records created (without embeddings).
    /// Non-text files are silently skipped (returns empty vec).
    ///
    /// # Arguments
    ///
    /// * `bot_id` - The bot that owns this file.
    /// * `file_id` - The file's UUID from the metadata store.
    /// * `filename` - The filename (used for MIME detection and markdown detection).
    /// * `content` - The raw file bytes.
    pub async fn index_file(
        &self,
        bot_id: &Uuid,
        file_id: &Uuid,
        filename: &str,
        content: &[u8],
    ) -> Result<Vec<FileChunk>, RepositoryError> {
        let mime = super::detect_mime(filename);
        if !super::is_text_mime(&mime) {
            return Ok(vec![]);
        }

        // Decode content as UTF-8
        let text = std::str::from_utf8(content)
            .map_err(|e| RepositoryError::Query(format!("File is not valid UTF-8: {e}")))?;

        if text.is_empty() {
            return Ok(vec![]);
        }

        // Chunk the text
        let ChunkResult { chunks, .. } = chunk_text_file(text, filename, None);

        if chunks.is_empty() {
            return Ok(vec![]);
        }

        // Generate embeddings for all chunks in one batch
        let chunk_texts: Vec<String> = chunks.clone();
        let embeddings = self.embedder.embed(&chunk_texts).await?;

        if embeddings.len() != chunks.len() {
            return Err(RepositoryError::Query(format!(
                "Embedding count ({}) doesn't match chunk count ({})",
                embeddings.len(),
                chunks.len()
            )));
        }

        // Build FileChunk records
        let model_name = self.embedder.model_name().to_string();
        let file_chunks: Vec<FileChunk> = chunks
            .iter()
            .enumerate()
            .map(|(i, chunk_text): (usize, &String)| FileChunk {
                chunk_id: Uuid::now_v7(),
                file_id: *file_id,
                bot_id: *bot_id,
                filename: filename.to_string(),
                chunk_index: i as u32,
                chunk_text: chunk_text.clone(),
                embedding_model: model_name.clone(),
            })
            .collect();

        // Store in LanceDB
        let table_name = LanceVectorStore::file_chunks_table_name(bot_id);
        let schema = Arc::new(file_chunks_schema());
        let table = self
            .vector_store
            .ensure_table(&table_name, schema.clone())
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to ensure table: {e}")))?;

        // Build Arrow RecordBatch and wrap in RecordBatchIterator
        let batch = build_chunks_batch(&file_chunks, &embeddings, &schema)?;
        let batch_iter = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(batch_iter)
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to insert chunks: {e}")))?;

        Ok(file_chunks)
    }

    /// Remove all chunks for a file from the vector store.
    pub async fn deindex_file(
        &self,
        bot_id: &Uuid,
        file_id: &Uuid,
    ) -> Result<(), RepositoryError> {
        let table_name = LanceVectorStore::file_chunks_table_name(bot_id);

        if !self.vector_store.table_exists(&table_name).await {
            return Ok(());
        }

        let table = self
            .vector_store
            .ensure_table(&table_name, Arc::new(file_chunks_schema()))
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to open table: {e}")))?;

        let filter = format!("file_id = '{}'", file_id);
        table
            .delete(&filter)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to delete chunks: {e}")))?;

        Ok(())
    }

    /// Re-index a file: remove old chunks and index new content.
    pub async fn reindex_file(
        &self,
        bot_id: &Uuid,
        file_id: &Uuid,
        filename: &str,
        content: &[u8],
    ) -> Result<Vec<FileChunk>, RepositoryError> {
        self.deindex_file(bot_id, file_id).await?;
        self.index_file(bot_id, file_id, filename, content).await
    }

    /// Search file chunks by semantic similarity.
    ///
    /// Returns up to `limit` chunks most similar to the query text.
    pub async fn search_file_chunks(
        &self,
        bot_id: &Uuid,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileChunk>, RepositoryError> {
        let table_name = LanceVectorStore::file_chunks_table_name(bot_id);

        if !self.vector_store.table_exists(&table_name).await {
            return Ok(vec![]);
        }

        // Embed the query
        let query_embedding = self
            .embedder
            .embed(&[query.to_string()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| RepositoryError::Query("Empty embedding result".to_string()))?;

        let table = self
            .vector_store
            .ensure_table(&table_name, Arc::new(file_chunks_schema()))
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to open table: {e}")))?;

        let results: Vec<RecordBatch> = table
            .vector_search(query_embedding)
            .map_err(|e| RepositoryError::Query(format!("Failed to search: {e}")))?
            .limit(limit)
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Search execution failed: {e}")))?
            .try_collect()
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to collect results: {e}")))?;

        let model_name = self.embedder.model_name().to_string();
        let mut chunks = Vec::new();

        for batch in &results {
            let num_rows = batch.num_rows();
            let chunk_id_col = get_string_col(batch, "chunk_id")?;
            let file_id_col = get_string_col(batch, "file_id")?;
            let bot_id_col = get_string_col(batch, "bot_id")?;
            let filename_col = get_string_col(batch, "filename")?;
            let chunk_index_col = batch
                .column_by_name("chunk_index")
                .ok_or_else(|| RepositoryError::Query("Missing chunk_index column".to_string()))?
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or_else(|| {
                    RepositoryError::Query("chunk_index is not an int32 array".to_string())
                })?;
            let chunk_text_col = get_string_col(batch, "chunk_text")?;

            for i in 0..num_rows {
                let chunk_id = Uuid::parse_str(chunk_id_col.value(i))
                    .map_err(|e| RepositoryError::Query(format!("Invalid chunk_id: {e}")))?;
                let file_id = Uuid::parse_str(file_id_col.value(i))
                    .map_err(|e| RepositoryError::Query(format!("Invalid file_id: {e}")))?;
                let bot_id = Uuid::parse_str(bot_id_col.value(i))
                    .map_err(|e| RepositoryError::Query(format!("Invalid bot_id: {e}")))?;

                chunks.push(FileChunk {
                    chunk_id,
                    file_id,
                    bot_id,
                    filename: filename_col.value(i).to_string(),
                    chunk_index: chunk_index_col.value(i) as u32,
                    chunk_text: chunk_text_col.value(i).to_string(),
                    embedding_model: model_name.clone(),
                });
            }
        }

        Ok(chunks)
    }
}

/// Extract a StringArray column from a RecordBatch.
fn get_string_col<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, RepositoryError> {
    batch
        .column_by_name(name)
        .ok_or_else(|| RepositoryError::Query(format!("Missing {name} column")))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| RepositoryError::Query(format!("{name} is not a string array")))
}

/// Build an Arrow RecordBatch from file chunks and their embeddings.
fn build_chunks_batch(
    chunks: &[FileChunk],
    embeddings: &[Vec<f32>],
    schema: &Schema,
) -> Result<RecordBatch, RepositoryError> {
    // Collect UUID strings first (need owned strings before borrowing)
    let chunk_id_strings: Vec<String> = chunks.iter().map(|c| c.chunk_id.to_string()).collect();
    let file_id_strings: Vec<String> = chunks.iter().map(|c| c.file_id.to_string()).collect();
    let bot_id_strings: Vec<String> = chunks.iter().map(|c| c.bot_id.to_string()).collect();

    let chunk_id_refs: Vec<&str> = chunk_id_strings.iter().map(|s| s.as_str()).collect();
    let file_id_refs: Vec<&str> = file_id_strings.iter().map(|s| s.as_str()).collect();
    let bot_id_refs: Vec<&str> = bot_id_strings.iter().map(|s| s.as_str()).collect();
    let filename_refs: Vec<&str> = chunks.iter().map(|c| c.filename.as_str()).collect();
    let chunk_indices: Vec<i32> = chunks.iter().map(|c| c.chunk_index as i32).collect();
    let chunk_text_refs: Vec<&str> = chunks.iter().map(|c| c.chunk_text.as_str()).collect();
    let model_refs: Vec<&str> = chunks.iter().map(|c| c.embedding_model.as_str()).collect();

    // Build the vector column as FixedSizeList of Float32
    let flat_values: Vec<f32> = embeddings.iter().flat_map(|e| e.iter().copied()).collect();
    let values_array = arrow_array::Float32Array::from(flat_values);
    let vector_field = Arc::new(Field::new("item", DataType::Float32, true));
    let vector_array = FixedSizeListArray::try_new(
        vector_field,
        EMBEDDING_DIMENSION,
        Arc::new(values_array),
        None,
    )
    .map_err(|e| RepositoryError::Query(format!("Failed to build vector array: {e}")))?;

    let batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![
            Arc::new(StringArray::from(chunk_id_refs)) as Arc<dyn Array>,
            Arc::new(StringArray::from(file_id_refs)),
            Arc::new(StringArray::from(bot_id_refs)),
            Arc::new(StringArray::from(filename_refs)),
            Arc::new(Int32Array::from(chunk_indices)),
            Arc::new(StringArray::from(chunk_text_refs)),
            Arc::new(StringArray::from(model_refs)),
            Arc::new(vector_array) as Arc<dyn Array>,
        ],
    )
    .map_err(|e| RepositoryError::Query(format!("Failed to build record batch: {e}")))?;

    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock embedder for testing that returns deterministic vectors.
    struct MockEmbedder {
        dimension: usize,
    }

    impl MockEmbedder {
        fn new() -> Self {
            Self {
                dimension: EMBEDDING_DIMENSION as usize,
            }
        }
    }

    impl Embedder for MockEmbedder {
        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RepositoryError> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    // Generate a deterministic vector based on text length and index
                    let mut vec = vec![0.0f32; self.dimension];
                    let seed = (text.len() as f32) * 0.01 + (i as f32) * 0.001;
                    for (j, v) in vec.iter_mut().enumerate() {
                        *v = ((j as f32 * seed).sin() * 0.5) + 0.5;
                    }
                    vec
                })
                .collect())
        }

        fn model_name(&self) -> &str {
            "mock-embedder"
        }

        fn dimension(&self) -> usize {
            self.dimension
        }
    }

    #[tokio::test]
    async fn test_index_text_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();
        let content = b"First section of the document.\n\nSecond section with more details.\n\nThird section wrapping up.";

        let chunks = indexer
            .index_file(&bot_id, &file_id, "notes.txt", content)
            .await
            .unwrap();

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.file_id == file_id));
        assert!(chunks.iter().all(|c| c.bot_id == bot_id));
        assert!(chunks.iter().all(|c| c.filename == "notes.txt"));
        assert_eq!(chunks[0].chunk_index, 0);
    }

    #[tokio::test]
    async fn test_index_markdown_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();
        let content = b"# Title\n\nIntro paragraph.\n\n## Section 1\n\nContent of section 1.\n\n## Section 2\n\nContent of section 2.";

        let chunks = indexer
            .index_file(&bot_id, &file_id, "readme.md", content)
            .await
            .unwrap();

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.embedding_model == "mock-embedder"));
    }

    #[tokio::test]
    async fn test_skip_binary_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();
        let content = b"\x89PNG\r\n\x1a\n"; // PNG magic bytes

        let chunks = indexer
            .index_file(&bot_id, &file_id, "image.png", content)
            .await
            .unwrap();

        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn test_index_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();

        let chunks = indexer
            .index_file(&bot_id, &file_id, "empty.txt", b"")
            .await
            .unwrap();

        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn test_deindex_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store.clone(), embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();
        let content = b"Some text content for indexing.";

        // Index first
        let chunks = indexer
            .index_file(&bot_id, &file_id, "test.txt", content)
            .await
            .unwrap();
        assert!(!chunks.is_empty());

        // Deindex
        indexer.deindex_file(&bot_id, &file_id).await.unwrap();

        // Verify table still exists but no matching chunks
        let table_name = LanceVectorStore::file_chunks_table_name(&bot_id);
        let table = vector_store
            .ensure_table(&table_name, Arc::new(file_chunks_schema()))
            .await
            .unwrap();

        let count = table.count_rows(None).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_deindex_nonexistent_table_ok() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();

        // Should not error even if table doesn't exist
        indexer.deindex_file(&bot_id, &file_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_reindex_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store.clone(), embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();

        // Index v1
        indexer
            .index_file(&bot_id, &file_id, "doc.txt", b"Version 1 content")
            .await
            .unwrap();

        // Reindex with v2
        let chunks = indexer
            .reindex_file(
                &bot_id,
                &file_id,
                "doc.txt",
                b"Version 2 content with more text and details",
            )
            .await
            .unwrap();

        assert!(!chunks.is_empty());

        // Verify the table has only v2 chunks
        let table_name = LanceVectorStore::file_chunks_table_name(&bot_id);
        let table = vector_store
            .ensure_table(&table_name, Arc::new(file_chunks_schema()))
            .await
            .unwrap();

        let count = table.count_rows(None).await.unwrap();
        assert_eq!(count, chunks.len());
    }

    #[tokio::test]
    async fn test_search_file_chunks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();

        // Index a document with enough content
        let content = b"Rust is a systems programming language focused on safety and performance.\n\nPython is great for data science and machine learning applications.\n\nJavaScript powers the modern web with frameworks like React and Vue.";

        indexer
            .index_file(&bot_id, &file_id, "langs.txt", content)
            .await
            .unwrap();

        // Search
        let results = indexer
            .search_file_chunks(&bot_id, "programming language", 5)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 5);
        assert!(results.iter().all(|c| c.bot_id == bot_id));
    }

    #[tokio::test]
    async fn test_search_empty_table() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vector_store = Arc::new(
            LanceVectorStore::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let embedder = Arc::new(MockEmbedder::new());
        let indexer = FileIndexer::new(vector_store, embedder);

        let bot_id = Uuid::now_v7();

        // Search when no table exists
        let results = indexer
            .search_file_chunks(&bot_id, "test query", 5)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_build_chunks_batch() {
        let bot_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();
        let chunks = vec![
            FileChunk {
                chunk_id: Uuid::now_v7(),
                file_id,
                bot_id,
                filename: "test.txt".to_string(),
                chunk_index: 0,
                chunk_text: "Hello world".to_string(),
                embedding_model: "test-model".to_string(),
            },
            FileChunk {
                chunk_id: Uuid::now_v7(),
                file_id,
                bot_id,
                filename: "test.txt".to_string(),
                chunk_index: 1,
                chunk_text: "Second chunk".to_string(),
                embedding_model: "test-model".to_string(),
            },
        ];

        let embeddings = vec![
            vec![0.1f32; EMBEDDING_DIMENSION as usize],
            vec![0.2f32; EMBEDDING_DIMENSION as usize],
        ];

        let schema = file_chunks_schema();
        let batch = build_chunks_batch(&chunks, &embeddings, &schema).unwrap();

        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 8);
    }
}
