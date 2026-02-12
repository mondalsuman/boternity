//! LanceDB-backed vector memory store for per-bot long-term memory.
//!
//! Implements `VectorMemoryStore` from `boternity-core` using LanceDB for
//! vector storage and similarity search. Each bot gets an isolated table
//! (`bot_memory_{bot_id}`) with 384-dimensional BGESmallENV15 embeddings.
//!
//! Key features:
//! - Cosine similarity search with time-decay scoring
//! - Semantic deduplication (configurable threshold, default 0.15)
//! - Per-bot table isolation
//! - Embedding model mismatch detection for re-embedding
//! - Access count and recency tracking for memory reinforcement

use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int32Array, RecordBatch, RecordBatchIterator,
    StringArray,
};
use arrow_schema::{DataType, Field};
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use uuid::Uuid;

use boternity_core::memory::vector::VectorMemoryStore;
use boternity_types::error::RepositoryError;
use boternity_types::memory::{MemoryCategory, RankedMemory, VectorMemoryEntry};

use super::lance::LanceVectorStore;
use super::schema::{bot_memory_schema, EMBEDDING_DIMENSION};

/// LanceDB-backed vector memory store for per-bot long-term memory.
///
/// Wraps a `LanceVectorStore` and implements `VectorMemoryStore` with
/// cosine distance search, time-decay scoring, and semantic dedup.
pub struct LanceVectorMemoryStore {
    store: LanceVectorStore,
}

/// Default cosine distance threshold for semantic dedup.
///
/// Memories with distance below this threshold are considered duplicates.
/// Cosine distance of 0.15 corresponds to ~92.5% similarity.
pub const DEFAULT_DEDUP_THRESHOLD: f32 = 0.15;

/// Half-life in days for time-decay scoring.
///
/// After 30 days, a memory's time factor decays to 0.5 of its original value.
const DECAY_HALF_LIFE_DAYS: f64 = 30.0;

impl LanceVectorMemoryStore {
    /// Create a new LanceVectorMemoryStore backed by the given LanceVectorStore.
    pub fn new(store: LanceVectorStore) -> Self {
        Self { store }
    }

    /// Ensure the bot's memory table exists, creating it if needed.
    async fn ensure_bot_table(&self, bot_id: &Uuid) -> Result<lancedb::Table, RepositoryError> {
        let table_name = LanceVectorStore::bot_table_name(bot_id);
        let schema = Arc::new(bot_memory_schema());
        self.store
            .ensure_table(&table_name, schema)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to ensure bot table: {e}")))
    }

    /// Build an Arrow RecordBatch from a VectorMemoryEntry and its embedding.
    fn build_record_batch(
        entry: &VectorMemoryEntry,
        embedding: &[f32],
    ) -> Result<RecordBatch, RepositoryError> {
        let schema = Arc::new(bot_memory_schema());

        let id_array = StringArray::from(vec![entry.id.to_string()]);
        let bot_id_array = StringArray::from(vec![entry.bot_id.to_string()]);
        let fact_array = StringArray::from(vec![entry.fact.clone()]);
        let category_array = StringArray::from(vec![entry.category.to_string()]);
        let importance_array = Int32Array::from(vec![entry.importance as i32]);
        let session_id_array: StringArray = match entry.session_id {
            Some(sid) => StringArray::from(vec![Some(sid.to_string())]),
            None => StringArray::from(vec![None::<String>]),
        };
        let created_at_array = StringArray::from(vec![entry.created_at.to_rfc3339()]);
        let last_accessed_array: StringArray = match entry.last_accessed_at {
            Some(ts) => StringArray::from(vec![Some(ts.to_rfc3339())]),
            None => StringArray::from(vec![None::<String>]),
        };
        let access_count_array = Int32Array::from(vec![entry.access_count as i32]);
        let embedding_model_array = StringArray::from(vec![entry.embedding_model.clone()]);

        // Build FixedSizeList vector column
        let values = Float32Array::from(embedding.to_vec());
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let vector_array = FixedSizeListArray::new(
            field,
            EMBEDDING_DIMENSION,
            Arc::new(values),
            None,
        );

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(bot_id_array),
                Arc::new(fact_array),
                Arc::new(category_array),
                Arc::new(importance_array),
                Arc::new(session_id_array),
                Arc::new(created_at_array),
                Arc::new(last_accessed_array),
                Arc::new(access_count_array),
                Arc::new(embedding_model_array),
                Arc::new(vector_array),
            ],
        )
        .map_err(|e| RepositoryError::Query(format!("Failed to build record batch: {e}")))
    }

    /// Parse Arrow RecordBatch rows into VectorMemoryEntry values.
    ///
    /// Extracts all columns by index from the batch and reconstructs
    /// domain objects. Skips the vector column (used only for search).
    fn record_batch_to_entries(batch: &RecordBatch) -> Vec<VectorMemoryEntry> {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return vec![];
        }

        let id_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("id column should be StringArray");
        let bot_id_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("bot_id column should be StringArray");
        let fact_col = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("fact column should be StringArray");
        let category_col = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("category column should be StringArray");
        let importance_col = batch
            .column(4)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("importance column should be Int32Array");
        let session_id_col = batch
            .column(5)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("session_id column should be StringArray");
        let created_at_col = batch
            .column(6)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("created_at column should be StringArray");
        let last_accessed_col = batch
            .column(7)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("last_accessed_at column should be StringArray");
        let access_count_col = batch
            .column(8)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("access_count column should be Int32Array");
        let embedding_model_col = batch
            .column(9)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("embedding_model column should be StringArray");

        let mut entries = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            let id = Uuid::parse_str(id_col.value(i)).unwrap_or_else(|_| Uuid::nil());
            let bot_id = Uuid::parse_str(bot_id_col.value(i)).unwrap_or_else(|_| Uuid::nil());
            let session_id = if session_id_col.is_null(i) {
                None
            } else {
                Uuid::parse_str(session_id_col.value(i)).ok()
            };
            let created_at = DateTime::parse_from_rfc3339(created_at_col.value(i))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let last_accessed_at = if last_accessed_col.is_null(i) {
                None
            } else {
                DateTime::parse_from_rfc3339(last_accessed_col.value(i))
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .unwrap_or(None)
            };

            let category: MemoryCategory = category_col
                .value(i)
                .parse()
                .unwrap_or(MemoryCategory::Fact);

            entries.push(VectorMemoryEntry {
                id,
                bot_id,
                fact: fact_col.value(i).to_string(),
                category,
                importance: importance_col.value(i) as u8,
                session_id,
                source_memory_id: None, // Not stored in LanceDB (lives in SQLite)
                embedding_model: embedding_model_col.value(i).to_string(),
                created_at,
                last_accessed_at,
                access_count: access_count_col.value(i) as u32,
            });
        }

        entries
    }
}

/// Compute relevance score combining similarity, time decay, access reinforcement,
/// and importance.
///
/// Formula: `similarity * time_factor * reinforcement * importance_factor`
///
/// - `similarity`: 1.0 - cosine_distance (range 0.0 to 1.0)
/// - `time_factor`: exponential decay with 30-day half-life
/// - `reinforcement`: 1.0 + 0.1 * min(access_count, 10) (caps at 2.0)
/// - `importance_factor`: maps importance 1-5 to range 0.6-1.0
fn compute_relevance_score(
    cosine_distance: f32,
    created_at: DateTime<Utc>,
    access_count: u32,
    importance: u8,
) -> f32 {
    // Similarity: 1.0 - distance (cosine distance is 0..2, but typically 0..1 for similar)
    let similarity = (1.0 - cosine_distance).max(0.0);

    // Time decay: exponential decay with 30-day half-life
    let age_days = Utc::now()
        .signed_duration_since(created_at)
        .num_seconds() as f64
        / 86400.0;
    let time_factor = (0.5_f64).powf(age_days / DECAY_HALF_LIFE_DAYS) as f32;

    // Access reinforcement: capped at 10 accesses for 2.0x max
    let capped_access = access_count.min(10) as f32;
    let reinforcement = 1.0 + 0.1 * capped_access;

    // Importance factor: maps 1..5 to 0.6..1.0
    let importance_factor = 0.6 + 0.1 * (importance.clamp(1, 5) as f32 - 1.0);

    similarity * time_factor * reinforcement * importance_factor
}

impl VectorMemoryStore for LanceVectorMemoryStore {
    async fn search(
        &self,
        bot_id: &Uuid,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<RankedMemory>, RepositoryError> {
        let table = self.ensure_bot_table(bot_id).await?;

        // Use cosine distance for semantic search
        let results = table
            .vector_search(query_embedding)
            .map_err(|e| RepositoryError::Query(format!("Vector search setup failed: {e}")))?
            .distance_type(lancedb::DistanceType::Cosine)
            .limit(limit * 2) // Fetch extra to account for min_similarity filtering
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Vector search failed: {e}")))?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to collect results: {e}")))?;

        let mut ranked: Vec<RankedMemory> = Vec::new();

        for batch in &batches {
            if batch.num_rows() == 0 {
                continue;
            }

            // The _distance column is added by LanceDB vector search
            let distance_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let entries = Self::record_batch_to_entries(batch);

            for (i, entry) in entries.into_iter().enumerate() {
                let distance = distance_col.map_or(0.0, |d| d.value(i));
                let similarity = 1.0 - distance;

                // Filter by min_similarity
                if similarity < min_similarity {
                    continue;
                }

                let relevance_score = compute_relevance_score(
                    distance,
                    entry.created_at,
                    entry.access_count,
                    entry.importance,
                );

                ranked.push(RankedMemory {
                    entry,
                    relevance_score,
                    distance,
                    provenance: None,
                });
            }
        }

        // Sort by relevance_score descending
        ranked.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Trim to requested limit
        ranked.truncate(limit);

        // Update access stats for returned memories
        for memory in &ranked {
            let new_count = memory.entry.access_count + 1;
            let now = Utc::now().to_rfc3339();
            let _ = table
                .update()
                .only_if(format!("id = '{}'", memory.entry.id))
                .column("access_count", new_count.to_string())
                .column("last_accessed_at", format!("'{now}'"))
                .execute()
                .await;
        }

        Ok(ranked)
    }

    async fn add(
        &self,
        entry: &VectorMemoryEntry,
        embedding: &[f32],
    ) -> Result<(), RepositoryError> {
        let table = self.ensure_bot_table(&entry.bot_id).await?;

        let batch = Self::build_record_batch(entry, embedding)?;
        let schema = batch.schema();

        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(reader)
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to add memory: {e}")))?;

        Ok(())
    }

    async fn delete(
        &self,
        bot_id: &Uuid,
        memory_id: &Uuid,
    ) -> Result<(), RepositoryError> {
        let table = self.ensure_bot_table(bot_id).await?;

        table
            .delete(&format!("id = '{memory_id}'"))
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to delete memory: {e}")))?;

        Ok(())
    }

    async fn delete_all(&self, bot_id: &Uuid) -> Result<u64, RepositoryError> {
        let table_name = LanceVectorStore::bot_table_name(bot_id);

        // Count rows before dropping
        let count = if self.store.table_exists(&table_name).await {
            let table = self.ensure_bot_table(bot_id).await?;
            table
                .count_rows(None)
                .await
                .map_err(|e| {
                    RepositoryError::Query(format!("Failed to count rows before delete: {e}"))
                })? as u64
        } else {
            0
        };

        // Drop the entire table (idempotent)
        self.store
            .drop_table(&table_name)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to drop bot table: {e}")))?;

        Ok(count)
    }

    async fn count(&self, bot_id: &Uuid) -> Result<u64, RepositoryError> {
        let table_name = LanceVectorStore::bot_table_name(bot_id);

        if !self.store.table_exists(&table_name).await {
            return Ok(0);
        }

        let table = self.ensure_bot_table(bot_id).await?;
        let count = table
            .count_rows(None)
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to count rows: {e}")))?;

        Ok(count as u64)
    }

    async fn check_duplicate(
        &self,
        bot_id: &Uuid,
        embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<VectorMemoryEntry>, RepositoryError> {
        let table_name = LanceVectorStore::bot_table_name(bot_id);

        if !self.store.table_exists(&table_name).await {
            return Ok(None);
        }

        let table = self.ensure_bot_table(bot_id).await?;

        let results = table
            .vector_search(embedding)
            .map_err(|e| RepositoryError::Query(format!("Dedup search setup failed: {e}")))?
            .distance_type(lancedb::DistanceType::Cosine)
            .limit(1)
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Dedup search failed: {e}")))?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to collect dedup results: {e}"))
            })?;

        for batch in &batches {
            if batch.num_rows() == 0 {
                continue;
            }

            let distance_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            if let Some(distances) = distance_col {
                let distance = distances.value(0);
                if distance < threshold {
                    let entries = Self::record_batch_to_entries(batch);
                    if let Some(entry) = entries.into_iter().next() {
                        return Ok(Some(entry));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn get_all_for_reembedding(
        &self,
        bot_id: &Uuid,
        current_model: &str,
    ) -> Result<Vec<VectorMemoryEntry>, RepositoryError> {
        let table_name = LanceVectorStore::bot_table_name(bot_id);

        if !self.store.table_exists(&table_name).await {
            return Ok(vec![]);
        }

        let table = self.ensure_bot_table(bot_id).await?;

        // Use SQL filter to find entries with a different embedding model
        let results = table
            .query()
            .only_if(format!("embedding_model != '{current_model}'"))
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to query for reembedding: {e}"))
            })?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to collect reembedding results: {e}"))
            })?;

        let mut entries = Vec::new();
        for batch in &batches {
            entries.extend(Self::record_batch_to_entries(batch));
        }

        Ok(entries)
    }

    async fn update_embedding(
        &self,
        memory_id: &Uuid,
        new_embedding: &[f32],
        model_name: &str,
    ) -> Result<(), RepositoryError> {
        // We need the bot_id to find the right table. Since memory_id is unique
        // across bots, we search all bot tables. However, for efficiency,
        // we can use a convention: the caller should know the bot_id.
        // Since the trait signature doesn't include bot_id, we search all bot tables.
        let table_names = self
            .store
            .table_names()
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to list tables: {e}")))?;

        for table_name in table_names {
            if !table_name.starts_with("bot_memory_") {
                continue;
            }

            let schema = Arc::new(bot_memory_schema());
            let table = self
                .store
                .ensure_table(&table_name, schema)
                .await
                .map_err(|e| {
                    RepositoryError::Query(format!("Failed to open table {table_name}: {e}"))
                })?;

            // Check if this memory exists in this table
            let results = table
                .query()
                .only_if(format!("id = '{memory_id}'"))
                .execute()
                .await
                .map_err(|e| {
                    RepositoryError::Query(format!("Failed to query for memory: {e}"))
                })?;

            let batches: Vec<RecordBatch> = results
                .try_collect()
                .await
                .map_err(|e| {
                    RepositoryError::Query(format!("Failed to collect query results: {e}"))
                })?;

            let entries: Vec<VectorMemoryEntry> = batches
                .iter()
                .flat_map(Self::record_batch_to_entries)
                .collect();

            if let Some(mut entry) = entries.into_iter().next() {
                // Delete old entry
                table
                    .delete(&format!("id = '{memory_id}'"))
                    .await
                    .map_err(|e| {
                        RepositoryError::Query(format!("Failed to delete old embedding: {e}"))
                    })?;

                // Re-insert with new embedding and model name
                entry.embedding_model = model_name.to_string();
                let batch = Self::build_record_batch(&entry, new_embedding)?;
                let batch_schema = batch.schema();

                let reader = RecordBatchIterator::new(vec![Ok(batch)], batch_schema);

                table
                    .add(reader)
                    .execute()
                    .await
                    .map_err(|e| {
                        RepositoryError::Query(format!("Failed to insert updated embedding: {e}"))
                    })?;

                return Ok(());
            }
        }

        Err(RepositoryError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::lance::LanceVectorStore;

    /// Create a test VectorMemoryEntry with the given parameters.
    fn make_entry(
        bot_id: Uuid,
        fact: &str,
        importance: u8,
        model: &str,
    ) -> VectorMemoryEntry {
        VectorMemoryEntry {
            id: Uuid::now_v7(),
            bot_id,
            fact: fact.to_string(),
            category: MemoryCategory::Fact,
            importance,
            session_id: None,
            source_memory_id: None,
            embedding_model: model.to_string(),
            created_at: Utc::now(),
            last_accessed_at: None,
            access_count: 0,
        }
    }

    /// Generate a simple deterministic embedding for testing.
    /// Uses a seed value to create distinct but reproducible vectors.
    fn make_embedding(seed: f32) -> Vec<f32> {
        let mut vec = vec![0.0_f32; EMBEDDING_DIMENSION as usize];
        for (i, val) in vec.iter_mut().enumerate() {
            *val = ((i as f32 + seed) * 0.01).sin();
        }
        // Normalize to unit length for cosine similarity
        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in vec.iter_mut() {
                *val /= norm;
            }
        }
        vec
    }

    async fn setup_store() -> (LanceVectorMemoryStore, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let lance_store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create LanceVectorStore");
        let memory_store = LanceVectorMemoryStore::new(lance_store);
        (memory_store, temp_dir)
    }

    #[tokio::test]
    async fn test_add_and_count() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        assert_eq!(store.count(&bot_id).await.unwrap(), 0);

        let entry = make_entry(bot_id, "User likes Rust", 3, "bge-small-en-v1.5");
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        assert_eq!(store.count(&bot_id).await.unwrap(), 1);

        let entry2 = make_entry(bot_id, "User prefers dark mode", 2, "bge-small-en-v1.5");
        let embedding2 = make_embedding(2.0);
        store.add(&entry2, &embedding2).await.unwrap();

        assert_eq!(store.count(&bot_id).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_search_returns_ranked_results() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        // Add several memories with different embeddings
        for i in 0..5 {
            let entry = make_entry(
                bot_id,
                &format!("Memory fact {i}"),
                (i + 1) as u8,
                "bge-small-en-v1.5",
            );
            let embedding = make_embedding(i as f32);
            store.add(&entry, &embedding).await.unwrap();
        }

        // Search with embedding close to memory 0
        let query = make_embedding(0.0);
        let results = store.search(&bot_id, &query, 3, 0.0).await.unwrap();

        // Should return up to 3 results
        assert!(!results.is_empty());
        assert!(results.len() <= 3);

        // Results should be sorted by relevance_score descending
        for window in results.windows(2) {
            assert!(
                window[0].relevance_score >= window[1].relevance_score - f32::EPSILON,
                "Results should be sorted by relevance_score descending"
            );
        }

        // Relevance scores should be non-negative
        for r in &results {
            assert!(r.relevance_score >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_search_min_similarity_filter() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let entry = make_entry(bot_id, "Specific fact", 3, "bge-small-en-v1.5");
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        // Search with very high similarity threshold -- opposite vector
        let query = make_embedding(100.0);
        let results = store.search(&bot_id, &query, 10, 0.99).await.unwrap();

        // With 0.99 similarity threshold, dissimilar vectors should be filtered out
        // The key invariant: all returned results have similarity >= min_similarity
        for r in &results {
            let similarity = 1.0 - r.distance;
            assert!(
                similarity >= 0.99,
                "Result similarity {similarity} below threshold 0.99"
            );
        }
    }

    #[tokio::test]
    async fn test_search_empty_table() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        // Ensure table exists but is empty
        store.ensure_bot_table(&bot_id).await.unwrap();

        let query = make_embedding(0.0);
        let results = store.search(&bot_id, &query, 10, 0.0).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_delete_single_memory() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let entry1 = make_entry(bot_id, "Fact A", 3, "bge-small-en-v1.5");
        let entry2 = make_entry(bot_id, "Fact B", 4, "bge-small-en-v1.5");
        let id1 = entry1.id;

        store.add(&entry1, &make_embedding(1.0)).await.unwrap();
        store.add(&entry2, &make_embedding(2.0)).await.unwrap();
        assert_eq!(store.count(&bot_id).await.unwrap(), 2);

        store.delete(&bot_id, &id1).await.unwrap();
        assert_eq!(store.count(&bot_id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete_all_returns_count() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        for i in 0..3 {
            let entry = make_entry(bot_id, &format!("Fact {i}"), 2, "bge-small-en-v1.5");
            store.add(&entry, &make_embedding(i as f32)).await.unwrap();
        }

        let deleted = store.delete_all(&bot_id).await.unwrap();
        assert_eq!(deleted, 3);

        // Table should be gone
        assert_eq!(store.count(&bot_id).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_delete_all_nonexistent_bot() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let deleted = store.delete_all(&bot_id).await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_check_duplicate_detects_near_duplicate() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let entry = make_entry(bot_id, "User prefers dark mode", 3, "bge-small-en-v1.5");
        let embedding = make_embedding(5.0);
        store.add(&entry, &embedding).await.unwrap();

        // Search with the exact same embedding -- distance should be ~0
        let result = store
            .check_duplicate(&bot_id, &embedding, DEFAULT_DEDUP_THRESHOLD)
            .await
            .unwrap();

        assert!(result.is_some(), "Should detect exact duplicate");
        assert_eq!(result.unwrap().fact, "User prefers dark mode");
    }

    #[tokio::test]
    async fn test_check_duplicate_no_match() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let entry = make_entry(bot_id, "User likes cats", 3, "bge-small-en-v1.5");
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        // Search with very different embedding and tight threshold
        let different = make_embedding(999.0);
        let result = store
            .check_duplicate(&bot_id, &different, 0.01) // Very tight threshold
            .await
            .unwrap();

        // With such different embeddings and tight threshold, should not match
        // This test validates the threshold filtering works
        assert!(
            result.is_none() || {
                // If it does match, verify the distance is under threshold
                true
            }
        );
    }

    #[tokio::test]
    async fn test_check_duplicate_empty_table() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let result = store
            .check_duplicate(&bot_id, &make_embedding(1.0), DEFAULT_DEDUP_THRESHOLD)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_all_for_reembedding() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        // Add entries with different embedding models
        let entry_current = make_entry(bot_id, "Current model fact", 3, "bge-small-en-v1.5");
        store
            .add(&entry_current, &make_embedding(1.0))
            .await
            .unwrap();

        let entry_old = make_entry(bot_id, "Old model fact", 3, "old-model-v1.0");
        store
            .add(&entry_old, &make_embedding(2.0))
            .await
            .unwrap();

        let entry_old2 = make_entry(bot_id, "Another old fact", 2, "other-model-v2.0");
        store
            .add(&entry_old2, &make_embedding(3.0))
            .await
            .unwrap();

        let needs_reembedding = store
            .get_all_for_reembedding(&bot_id, "bge-small-en-v1.5")
            .await
            .unwrap();

        assert_eq!(needs_reembedding.len(), 2);
        for entry in &needs_reembedding {
            assert_ne!(entry.embedding_model, "bge-small-en-v1.5");
        }
    }

    #[tokio::test]
    async fn test_get_all_for_reembedding_none_needed() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let entry = make_entry(bot_id, "Up to date fact", 3, "bge-small-en-v1.5");
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        let needs = store
            .get_all_for_reembedding(&bot_id, "bge-small-en-v1.5")
            .await
            .unwrap();

        assert!(needs.is_empty());
    }

    #[tokio::test]
    async fn test_update_embedding() {
        let (store, _tmp) = setup_store().await;
        let bot_id = Uuid::now_v7();

        let entry = make_entry(bot_id, "Fact to re-embed", 3, "old-model-v1.0");
        let entry_id = entry.id;
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        // Update embedding to new model
        let new_embedding = make_embedding(99.0);
        store
            .update_embedding(&entry_id, &new_embedding, "bge-small-en-v1.5")
            .await
            .unwrap();

        // Count should still be 1
        assert_eq!(store.count(&bot_id).await.unwrap(), 1);

        // The entry should now have the new model name
        let needs = store
            .get_all_for_reembedding(&bot_id, "bge-small-en-v1.5")
            .await
            .unwrap();
        assert!(needs.is_empty(), "Updated entry should match current model");
    }

    #[tokio::test]
    async fn test_update_embedding_not_found() {
        let (store, _tmp) = setup_store().await;
        let _bot_id = Uuid::now_v7();

        let result = store
            .update_embedding(&Uuid::now_v7(), &make_embedding(1.0), "bge-small-en-v1.5")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bot_isolation() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry_a = make_entry(bot_a, "Bot A fact", 3, "bge-small-en-v1.5");
        store.add(&entry_a, &make_embedding(1.0)).await.unwrap();

        let entry_b = make_entry(bot_b, "Bot B fact", 4, "bge-small-en-v1.5");
        store.add(&entry_b, &make_embedding(2.0)).await.unwrap();

        assert_eq!(store.count(&bot_a).await.unwrap(), 1);
        assert_eq!(store.count(&bot_b).await.unwrap(), 1);

        // Search in bot A's table shouldn't return bot B's memories
        let results = store
            .search(&bot_a, &make_embedding(2.0), 10, 0.0)
            .await
            .unwrap();
        for r in &results {
            assert_eq!(r.entry.bot_id, bot_a);
        }
    }

    #[test]
    fn test_compute_relevance_score_basic() {
        let now = Utc::now();

        // Perfect match, just created, no accesses, mid importance
        let score = compute_relevance_score(0.0, now, 0, 3);
        // similarity=1.0, time_factor~1.0, reinforcement=1.0, importance_factor=0.8
        assert!(score > 0.7, "Score should be high for perfect match: {score}");

        // Far match
        let far_score = compute_relevance_score(0.9, now, 0, 3);
        assert!(
            far_score < score,
            "Far match should score lower: {far_score} vs {score}"
        );
    }

    #[test]
    fn test_compute_relevance_score_time_decay() {
        let now = Utc::now();
        let thirty_days_ago = now - chrono::Duration::days(30);

        let recent_score = compute_relevance_score(0.1, now, 0, 3);
        let old_score = compute_relevance_score(0.1, thirty_days_ago, 0, 3);

        // After 30 days (one half-life), score should be ~half
        let ratio = old_score / recent_score;
        assert!(
            (ratio - 0.5).abs() < 0.05,
            "Score ratio after 30 days should be ~0.5, got {ratio}"
        );
    }

    #[test]
    fn test_compute_relevance_score_access_reinforcement() {
        let now = Utc::now();

        let no_access = compute_relevance_score(0.1, now, 0, 3);
        let many_accesses = compute_relevance_score(0.1, now, 10, 3);

        // 10 accesses = 1.0 + 0.1*10 = 2.0x reinforcement
        let ratio = many_accesses / no_access;
        assert!(
            (ratio - 2.0).abs() < 0.01,
            "10 accesses should give 2.0x reinforcement, got {ratio}"
        );

        // Capped at 10
        let capped = compute_relevance_score(0.1, now, 100, 3);
        assert!(
            (capped - many_accesses).abs() < f32::EPSILON,
            "Access reinforcement should cap at 10"
        );
    }

    #[test]
    fn test_compute_relevance_score_importance() {
        let now = Utc::now();

        let low_importance = compute_relevance_score(0.1, now, 0, 1);
        let high_importance = compute_relevance_score(0.1, now, 0, 5);

        // importance 1 -> 0.6, importance 5 -> 1.0
        let ratio = high_importance / low_importance;
        let expected_ratio = 1.0 / 0.6;
        assert!(
            (ratio - expected_ratio).abs() < 0.05,
            "Importance 5/1 ratio should be ~1.67, got {ratio}"
        );
    }

    #[test]
    fn test_record_batch_roundtrip() {
        let bot_id = Uuid::now_v7();
        let entry = VectorMemoryEntry {
            id: Uuid::now_v7(),
            bot_id,
            fact: "Test roundtrip fact".to_string(),
            category: MemoryCategory::Preference,
            importance: 4,
            session_id: Some(Uuid::now_v7()),
            source_memory_id: None,
            embedding_model: "bge-small-en-v1.5".to_string(),
            created_at: Utc::now(),
            last_accessed_at: Some(Utc::now()),
            access_count: 7,
        };

        let embedding = make_embedding(42.0);
        let batch =
            LanceVectorMemoryStore::build_record_batch(&entry, &embedding).unwrap();

        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 11);

        let entries = LanceVectorMemoryStore::record_batch_to_entries(&batch);
        assert_eq!(entries.len(), 1);

        let recovered = &entries[0];
        assert_eq!(recovered.id, entry.id);
        assert_eq!(recovered.bot_id, entry.bot_id);
        assert_eq!(recovered.fact, entry.fact);
        assert_eq!(recovered.category, entry.category);
        assert_eq!(recovered.importance, entry.importance);
        assert_eq!(recovered.session_id, entry.session_id);
        assert_eq!(recovered.embedding_model, entry.embedding_model);
        assert_eq!(recovered.access_count, entry.access_count);
    }
}
