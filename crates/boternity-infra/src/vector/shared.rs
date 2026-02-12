//! LanceDB-backed shared memory store for cross-bot knowledge sharing.
//!
//! Implements `SharedMemoryStore` from `boternity-core` using a single global
//! LanceDB table (`shared_memory`) with trust-level partitioning.
//!
//! Key features:
//! - Trust-level filtering: Public (all bots), Trusted (explicit trust list), Private (author only)
//! - Provenance tracking: "Written by BotX" annotation on all shared memories
//! - SHA-256 tamper detection hash on every write
//! - Per-bot contribution cap (default 500)
//! - Author-only deletion and revocation

use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int32Array, RecordBatch, RecordBatchIterator,
    StringArray,
};
use arrow_schema::{DataType, Field};
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use boternity_core::memory::shared::SharedMemoryStore;
use boternity_types::error::RepositoryError;
use boternity_types::memory::{
    MemoryCategory, RankedMemory, SharedMemoryEntry, TrustLevel, VectorMemoryEntry,
};

use super::lance::LanceVectorStore;
use super::schema::{shared_memory_schema, EMBEDDING_DIMENSION};

/// Default per-bot contribution cap for shared memories.
pub const DEFAULT_CONTRIBUTION_CAP: u64 = 500;

/// LanceDB-backed shared memory store for cross-bot knowledge sharing.
///
/// Wraps a `LanceVectorStore` and implements `SharedMemoryStore` with
/// trust-level filtering, provenance tracking, and SHA-256 integrity.
pub struct LanceSharedMemoryStore {
    store: LanceVectorStore,
    contribution_cap: u64,
}

impl LanceSharedMemoryStore {
    /// Create a new LanceSharedMemoryStore with the default contribution cap.
    pub fn new(store: LanceVectorStore) -> Self {
        Self {
            store,
            contribution_cap: DEFAULT_CONTRIBUTION_CAP,
        }
    }

    /// Create a new LanceSharedMemoryStore with a custom contribution cap.
    pub fn with_cap(store: LanceVectorStore, cap: u64) -> Self {
        Self {
            store,
            contribution_cap: cap,
        }
    }

    /// Ensure the shared memory table exists, creating it if needed.
    async fn ensure_shared_table(&self) -> Result<lancedb::Table, RepositoryError> {
        let table_name = LanceVectorStore::shared_table_name();
        let schema = Arc::new(shared_memory_schema());
        self.store
            .ensure_table(table_name, schema)
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to ensure shared memory table: {e}"))
            })
    }

    /// Compute the SHA-256 write hash for tamper detection.
    ///
    /// Hash covers: id + fact + category + importance + author_bot_id + trust_level + created_at
    fn compute_write_hash(entry: &SharedMemoryEntry) -> String {
        let mut hasher = Sha256::new();
        hasher.update(entry.id.to_string().as_bytes());
        hasher.update(entry.fact.as_bytes());
        hasher.update(entry.category.to_string().as_bytes());
        hasher.update(entry.importance.to_string().as_bytes());
        hasher.update(entry.author_bot_id.to_string().as_bytes());
        hasher.update(entry.trust_level.to_string().as_bytes());
        hasher.update(entry.created_at.to_rfc3339().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Build an Arrow RecordBatch from a SharedMemoryEntry and its embedding.
    fn build_record_batch(
        entry: &SharedMemoryEntry,
        embedding: &[f32],
    ) -> Result<RecordBatch, RepositoryError> {
        let schema = Arc::new(shared_memory_schema());

        let id_array = StringArray::from(vec![entry.id.to_string()]);
        let fact_array = StringArray::from(vec![entry.fact.clone()]);
        let category_array = StringArray::from(vec![entry.category.to_string()]);
        let importance_array = Int32Array::from(vec![entry.importance as i32]);
        let author_bot_id_array = StringArray::from(vec![entry.author_bot_id.to_string()]);
        let author_bot_name_array = StringArray::from(vec![entry.author_bot_name.clone()]);
        let trust_level_array = StringArray::from(vec![entry.trust_level.to_string()]);
        let created_at_array = StringArray::from(vec![entry.created_at.to_rfc3339()]);
        let write_hash_array = StringArray::from(vec![entry.write_hash.clone()]);
        let embedding_model_array = StringArray::from(vec![entry.embedding_model.clone()]);

        // Build FixedSizeList vector column
        let values = Float32Array::from(embedding.to_vec());
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let vector_array =
            FixedSizeListArray::new(field, EMBEDDING_DIMENSION, Arc::new(values), None);

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(fact_array),
                Arc::new(category_array),
                Arc::new(importance_array),
                Arc::new(author_bot_id_array),
                Arc::new(author_bot_name_array),
                Arc::new(trust_level_array),
                Arc::new(created_at_array),
                Arc::new(write_hash_array),
                Arc::new(embedding_model_array),
                Arc::new(vector_array),
            ],
        )
        .map_err(|e| RepositoryError::Query(format!("Failed to build shared record batch: {e}")))
    }

    /// Parse Arrow RecordBatch rows into SharedMemoryEntry values.
    fn record_batch_to_entries(batch: &RecordBatch) -> Vec<SharedMemoryEntry> {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return vec![];
        }

        let id_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("id column should be StringArray");
        let fact_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("fact column should be StringArray");
        let category_col = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("category column should be StringArray");
        let importance_col = batch
            .column(3)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("importance column should be Int32Array");
        let author_bot_id_col = batch
            .column(4)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("author_bot_id column should be StringArray");
        let author_bot_name_col = batch
            .column(5)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("author_bot_name column should be StringArray");
        let trust_level_col = batch
            .column(6)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("trust_level column should be StringArray");
        let created_at_col = batch
            .column(7)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("created_at column should be StringArray");
        let write_hash_col = batch
            .column(8)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("write_hash column should be StringArray");
        let embedding_model_col = batch
            .column(9)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("embedding_model column should be StringArray");

        let mut entries = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            let id = Uuid::parse_str(id_col.value(i)).unwrap_or_else(|_| Uuid::nil());
            let author_bot_id =
                Uuid::parse_str(author_bot_id_col.value(i)).unwrap_or_else(|_| Uuid::nil());
            let created_at = DateTime::parse_from_rfc3339(created_at_col.value(i))
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let category: MemoryCategory = category_col
                .value(i)
                .parse()
                .unwrap_or(MemoryCategory::Fact);
            let trust_level: TrustLevel = trust_level_col
                .value(i)
                .parse()
                .unwrap_or(TrustLevel::Private);

            entries.push(SharedMemoryEntry {
                id,
                fact: fact_col.value(i).to_string(),
                category,
                importance: importance_col.value(i) as u8,
                author_bot_id,
                author_bot_name: author_bot_name_col.value(i).to_string(),
                trust_level,
                embedding_model: embedding_model_col.value(i).to_string(),
                write_hash: write_hash_col.value(i).to_string(),
                created_at,
            });
        }

        entries
    }

    /// Convert a SharedMemoryEntry to a VectorMemoryEntry for RankedMemory.
    ///
    /// Shared memories are represented as VectorMemoryEntry within RankedMemory
    /// so that private and shared results can be merged into a single ranking.
    fn to_vector_entry(entry: &SharedMemoryEntry) -> VectorMemoryEntry {
        VectorMemoryEntry {
            id: entry.id,
            bot_id: entry.author_bot_id,
            fact: entry.fact.clone(),
            category: entry.category.clone(),
            importance: entry.importance,
            session_id: None,
            source_memory_id: None,
            embedding_model: entry.embedding_model.clone(),
            created_at: entry.created_at,
            last_accessed_at: None,
            access_count: 0,
        }
    }

    /// Build the trust filter SQL expression for LanceDB queries.
    ///
    /// A bot can see:
    /// - All public memories
    /// - Its own memories (any trust level)
    /// - Trusted memories from bots in its trust list
    fn build_trust_filter(reading_bot_id: &Uuid, trusted_bot_ids: &[Uuid]) -> String {
        let reading_id = reading_bot_id.to_string();

        let mut parts = vec![
            // Public memories visible to everyone
            "trust_level = 'public'".to_string(),
            // Author's own memories always visible
            format!("author_bot_id = '{reading_id}'"),
        ];

        // Trusted memories from explicitly trusted bots
        if !trusted_bot_ids.is_empty() {
            let trusted_ids: Vec<String> = trusted_bot_ids
                .iter()
                .map(|id| format!("'{id}'"))
                .collect();
            parts.push(format!(
                "(trust_level = 'trusted' AND author_bot_id IN ({}))",
                trusted_ids.join(", ")
            ));
        }

        parts.join(" OR ")
    }
}

impl SharedMemoryStore for LanceSharedMemoryStore {
    async fn add(
        &self,
        entry: &SharedMemoryEntry,
        embedding: &[f32],
    ) -> Result<(), RepositoryError> {
        // Enforce per-bot contribution cap
        let count = self.count_by_author(&entry.author_bot_id).await?;
        if count >= self.contribution_cap {
            return Err(RepositoryError::Query(format!(
                "Per-bot contribution cap reached ({}/{}). Delete existing shared memories before adding more.",
                count, self.contribution_cap
            )));
        }

        let table = self.ensure_shared_table().await?;

        // Compute write hash if not already set (empty or placeholder)
        let mut entry_with_hash = entry.clone();
        if entry_with_hash.write_hash.is_empty() {
            entry_with_hash.write_hash = Self::compute_write_hash(&entry_with_hash);
        }

        let batch = Self::build_record_batch(&entry_with_hash, embedding)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(reader)
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Failed to add shared memory: {e}")))?;

        Ok(())
    }

    async fn search(
        &self,
        reading_bot_id: &Uuid,
        trusted_bot_ids: &[Uuid],
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<RankedMemory>, RepositoryError> {
        let table = self.ensure_shared_table().await?;

        let trust_filter = Self::build_trust_filter(reading_bot_id, trusted_bot_ids);

        let results = table
            .vector_search(query_embedding)
            .map_err(|e| {
                RepositoryError::Query(format!("Shared vector search setup failed: {e}"))
            })?
            .distance_type(lancedb::DistanceType::Cosine)
            .only_if(trust_filter)
            .limit(limit * 2) // Over-fetch for filtering
            .execute()
            .await
            .map_err(|e| RepositoryError::Query(format!("Shared vector search failed: {e}")))?;

        let batches: Vec<RecordBatch> = results.try_collect().await.map_err(|e| {
            RepositoryError::Query(format!("Failed to collect shared search results: {e}"))
        })?;

        let mut ranked: Vec<RankedMemory> = Vec::new();

        for batch in &batches {
            if batch.num_rows() == 0 {
                continue;
            }

            let distance_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let entries = Self::record_batch_to_entries(batch);

            for (i, entry) in entries.into_iter().enumerate() {
                let distance = distance_col.map_or(0.0, |d| d.value(i));
                let similarity = 1.0 - distance;

                if similarity < min_similarity {
                    continue;
                }

                // Use similarity directly as relevance score for shared memories
                // (no time-decay or access tracking for shared memories)
                let relevance_score = similarity;

                // Build provenance annotation
                let provenance = Some(format!("Written by {}", entry.author_bot_name));

                ranked.push(RankedMemory {
                    entry: Self::to_vector_entry(&entry),
                    relevance_score,
                    distance,
                    provenance,
                });
            }
        }

        // Sort by relevance descending
        ranked.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ranked.truncate(limit);
        Ok(ranked)
    }

    async fn delete(
        &self,
        memory_id: &Uuid,
        author_bot_id: &Uuid,
    ) -> Result<(), RepositoryError> {
        let table = self.ensure_shared_table().await?;

        // Author-only deletion: filter by both id and author_bot_id
        table
            .delete(&format!(
                "id = '{}' AND author_bot_id = '{}'",
                memory_id, author_bot_id
            ))
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to delete shared memory: {e}"))
            })?;

        Ok(())
    }

    async fn share(
        &self,
        memory_id: &Uuid,
        trust_level: TrustLevel,
    ) -> Result<(), RepositoryError> {
        let table = self.ensure_shared_table().await?;

        // Read the existing entry
        let results = table
            .query()
            .only_if(format!("id = '{memory_id}'"))
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to query shared memory for share: {e}"))
            })?;

        let batches: Vec<RecordBatch> = results.try_collect().await.map_err(|e| {
            RepositoryError::Query(format!("Failed to collect share query results: {e}"))
        })?;

        let entries: Vec<SharedMemoryEntry> = batches
            .iter()
            .flat_map(Self::record_batch_to_entries)
            .collect();

        let entry = entries
            .into_iter()
            .next()
            .ok_or(RepositoryError::NotFound)?;

        // Also read the vector from the batch to preserve it
        let embedding = Self::extract_embedding_from_batch(&batches[0], 0)?;

        // Delete the old entry
        table
            .delete(&format!("id = '{memory_id}'"))
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to delete old shared memory: {e}"))
            })?;

        // Re-insert with updated trust level and recomputed hash
        let mut updated = entry;
        updated.trust_level = trust_level;
        updated.write_hash = Self::compute_write_hash(&updated);

        let batch = Self::build_record_batch(&updated, &embedding)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(reader)
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to re-insert shared memory: {e}"))
            })?;

        Ok(())
    }

    async fn revoke(
        &self,
        memory_id: &Uuid,
        author_bot_id: &Uuid,
    ) -> Result<(), RepositoryError> {
        let table = self.ensure_shared_table().await?;

        // Read the existing entry, verifying authorship
        let results = table
            .query()
            .only_if(format!(
                "id = '{}' AND author_bot_id = '{}'",
                memory_id, author_bot_id
            ))
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to query shared memory for revoke: {e}"))
            })?;

        let batches: Vec<RecordBatch> = results.try_collect().await.map_err(|e| {
            RepositoryError::Query(format!("Failed to collect revoke query results: {e}"))
        })?;

        if batches.is_empty() || batches[0].num_rows() == 0 {
            return Err(RepositoryError::NotFound);
        }

        let entries: Vec<SharedMemoryEntry> = batches
            .iter()
            .flat_map(Self::record_batch_to_entries)
            .collect();

        let entry = entries
            .into_iter()
            .next()
            .ok_or(RepositoryError::NotFound)?;

        let embedding = Self::extract_embedding_from_batch(&batches[0], 0)?;

        // Delete the old entry
        table
            .delete(&format!(
                "id = '{}' AND author_bot_id = '{}'",
                memory_id, author_bot_id
            ))
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to delete shared memory for revoke: {e}"))
            })?;

        // Re-insert with Private trust level
        let mut updated = entry;
        updated.trust_level = TrustLevel::Private;
        updated.write_hash = Self::compute_write_hash(&updated);

        let batch = Self::build_record_batch(&updated, &embedding)?;
        let schema = batch.schema();
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(reader)
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!(
                    "Failed to re-insert revoked shared memory: {e}"
                ))
            })?;

        Ok(())
    }

    async fn count_by_author(&self, author_bot_id: &Uuid) -> Result<u64, RepositoryError> {
        let table_name = LanceVectorStore::shared_table_name();

        if !self.store.table_exists(table_name).await {
            return Ok(0);
        }

        let table = self.ensure_shared_table().await?;

        let results = table
            .query()
            .only_if(format!("author_bot_id = '{author_bot_id}'"))
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!("Failed to count shared memories by author: {e}"))
            })?;

        let batches: Vec<RecordBatch> = results.try_collect().await.map_err(|e| {
            RepositoryError::Query(format!("Failed to collect count query results: {e}"))
        })?;

        let count: usize = batches.iter().map(|b| b.num_rows()).sum();
        Ok(count as u64)
    }

    async fn verify_integrity(&self, memory_id: &Uuid) -> Result<bool, RepositoryError> {
        let table = self.ensure_shared_table().await?;

        let results = table
            .query()
            .only_if(format!("id = '{memory_id}'"))
            .execute()
            .await
            .map_err(|e| {
                RepositoryError::Query(format!(
                    "Failed to query shared memory for integrity check: {e}"
                ))
            })?;

        let batches: Vec<RecordBatch> = results.try_collect().await.map_err(|e| {
            RepositoryError::Query(format!(
                "Failed to collect integrity check query results: {e}"
            ))
        })?;

        let entries: Vec<SharedMemoryEntry> = batches
            .iter()
            .flat_map(Self::record_batch_to_entries)
            .collect();

        let entry = entries
            .into_iter()
            .next()
            .ok_or(RepositoryError::NotFound)?;

        let expected_hash = Self::compute_write_hash(&entry);
        Ok(entry.write_hash == expected_hash)
    }
}

impl LanceSharedMemoryStore {
    /// Extract the embedding vector from a RecordBatch at a given row index.
    ///
    /// The vector column is the last column (index 10) in the shared_memory schema.
    fn extract_embedding_from_batch(
        batch: &RecordBatch,
        row_index: usize,
    ) -> Result<Vec<f32>, RepositoryError> {
        let vector_col = batch
            .column(10)
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or_else(|| {
                RepositoryError::Query("vector column is not FixedSizeListArray".to_string())
            })?;

        let value_array = vector_col.value(row_index);
        let values = value_array
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| {
                RepositoryError::Query("vector values are not Float32Array".to_string())
            })?;

        Ok(values.values().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::lance::LanceVectorStore;

    /// Create a test SharedMemoryEntry.
    fn make_shared_entry(
        author_bot_id: Uuid,
        author_name: &str,
        fact: &str,
        trust_level: TrustLevel,
        importance: u8,
    ) -> SharedMemoryEntry {
        let mut entry = SharedMemoryEntry {
            id: Uuid::now_v7(),
            fact: fact.to_string(),
            category: MemoryCategory::Fact,
            importance,
            author_bot_id,
            author_bot_name: author_name.to_string(),
            trust_level,
            embedding_model: "bge-small-en-v1.5".to_string(),
            write_hash: String::new(),
            created_at: Utc::now(),
        };
        entry.write_hash = LanceSharedMemoryStore::compute_write_hash(&entry);
        entry
    }

    /// Generate a simple deterministic embedding for testing.
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

    async fn setup_store() -> (LanceSharedMemoryStore, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let lance_store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create LanceVectorStore");
        let shared_store = LanceSharedMemoryStore::new(lance_store);
        (shared_store, temp_dir)
    }

    async fn setup_store_with_cap(cap: u64) -> (LanceSharedMemoryStore, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let lance_store = LanceVectorStore::new(temp_dir.path().to_path_buf())
            .await
            .expect("Failed to create LanceVectorStore");
        let shared_store = LanceSharedMemoryStore::with_cap(lance_store, cap);
        (shared_store, temp_dir)
    }

    // --- CRUD Tests ---

    #[tokio::test]
    async fn test_add_and_count_by_author() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();

        assert_eq!(store.count_by_author(&bot_a).await.unwrap(), 0);

        let entry = make_shared_entry(bot_a, "BotA", "Shared fact 1", TrustLevel::Public, 3);
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        assert_eq!(store.count_by_author(&bot_a).await.unwrap(), 1);

        let entry2 = make_shared_entry(bot_a, "BotA", "Shared fact 2", TrustLevel::Trusted, 4);
        store.add(&entry2, &make_embedding(2.0)).await.unwrap();

        assert_eq!(store.count_by_author(&bot_a).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_delete_author_only() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry = make_shared_entry(bot_a, "BotA", "Secret knowledge", TrustLevel::Public, 3);
        let entry_id = entry.id;
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        // Bot B should not be able to delete Bot A's memory
        store.delete(&entry_id, &bot_b).await.unwrap(); // No error, just no-op
        assert_eq!(
            store.count_by_author(&bot_a).await.unwrap(),
            1,
            "Bot B should not be able to delete Bot A's memory"
        );

        // Bot A should be able to delete its own memory
        store.delete(&entry_id, &bot_a).await.unwrap();
        assert_eq!(store.count_by_author(&bot_a).await.unwrap(), 0);
    }

    // --- Trust Level Filtering Tests ---

    #[tokio::test]
    async fn test_public_visible_to_all() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry =
            make_shared_entry(bot_a, "BotA", "Public knowledge", TrustLevel::Public, 3);
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        // Bot B (not in any trust list) should see public memories
        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.fact, "Public knowledge");
        assert_eq!(
            results[0].provenance,
            Some("Written by BotA".to_string())
        );
    }

    #[tokio::test]
    async fn test_trusted_visible_to_trust_list() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();
        let bot_c = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "BotA",
            "Trusted-only knowledge",
            TrustLevel::Trusted,
            4,
        );
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        // Bot B trusts Bot A -- should see the trusted memory
        let results = store
            .search(&bot_b, &[bot_a], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.fact, "Trusted-only knowledge");

        // Bot C does NOT trust Bot A -- should NOT see the trusted memory
        let results = store
            .search(&bot_c, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert!(
            results.is_empty(),
            "Bot C should not see Bot A's trusted memory"
        );
    }

    #[tokio::test]
    async fn test_private_visible_to_author_only() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "BotA",
            "Private knowledge",
            TrustLevel::Private,
            5,
        );
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        // Author (Bot A) should see their own private memory
        let results = store
            .search(&bot_a, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.fact, "Private knowledge");

        // Bot B should NOT see Bot A's private memory
        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert!(
            results.is_empty(),
            "Bot B should not see Bot A's private memory"
        );

        // Bot B with trust list including Bot A still should NOT see private
        let results = store
            .search(&bot_b, &[bot_a], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert!(
            results.is_empty(),
            "Trust list should not grant access to private memories"
        );
    }

    // --- SHA-256 Integrity Tests ---

    #[tokio::test]
    async fn test_integrity_check_passes() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();

        let entry = make_shared_entry(bot_a, "BotA", "Tamper-proof fact", TrustLevel::Public, 3);
        let entry_id = entry.id;
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        let result = store.verify_integrity(&entry_id).await.unwrap();
        assert!(result, "Integrity check should pass for unmodified entry");
    }

    #[tokio::test]
    async fn test_integrity_check_not_found() {
        let (store, _tmp) = setup_store().await;

        // Ensure the shared table exists (empty)
        store.ensure_shared_table().await.unwrap();

        let result = store.verify_integrity(&Uuid::now_v7()).await;
        assert!(result.is_err(), "Should error for non-existent memory");
    }

    // --- Share / Revoke Tests ---

    #[tokio::test]
    async fn test_share_changes_trust_level() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "BotA",
            "Initially private",
            TrustLevel::Private,
            3,
        );
        let entry_id = entry.id;
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        // Bot B can't see it
        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert!(results.is_empty());

        // Share as public
        store.share(&entry_id, TrustLevel::Public).await.unwrap();

        // Now Bot B can see it
        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.fact, "Initially private");
    }

    #[tokio::test]
    async fn test_revoke_sets_private() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "BotA",
            "Originally public",
            TrustLevel::Public,
            3,
        );
        let entry_id = entry.id;
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        // Bot B can see it initially
        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Author revokes sharing
        store.revoke(&entry_id, &bot_a).await.unwrap();

        // Bot B can no longer see it
        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert!(results.is_empty(), "Revoked memory should not be visible");

        // Author can still see it
        let results = store
            .search(&bot_a, &[], &embedding, 10, 0.0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_revoke_non_author_fails() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry = make_shared_entry(bot_a, "BotA", "Public data", TrustLevel::Public, 3);
        let entry_id = entry.id;
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        // Bot B should not be able to revoke Bot A's memory
        let result = store.revoke(&entry_id, &bot_b).await;
        assert!(result.is_err(), "Non-author should not be able to revoke");
    }

    // --- Per-bot Contribution Cap Tests ---

    #[tokio::test]
    async fn test_contribution_cap_enforced() {
        let (store, _tmp) = setup_store_with_cap(3).await;
        let bot_a = Uuid::now_v7();

        for i in 0..3 {
            let entry = make_shared_entry(
                bot_a,
                "BotA",
                &format!("Fact {i}"),
                TrustLevel::Public,
                3,
            );
            store.add(&entry, &make_embedding(i as f32)).await.unwrap();
        }

        assert_eq!(store.count_by_author(&bot_a).await.unwrap(), 3);

        // 4th entry should be rejected
        let entry4 = make_shared_entry(bot_a, "BotA", "One too many", TrustLevel::Public, 3);
        let result = store.add(&entry4, &make_embedding(99.0)).await;
        assert!(result.is_err(), "Should reject when cap is reached");
    }

    #[tokio::test]
    async fn test_cap_is_per_bot() {
        let (store, _tmp) = setup_store_with_cap(2).await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        for i in 0..2 {
            let entry_a = make_shared_entry(
                bot_a,
                "BotA",
                &format!("A fact {i}"),
                TrustLevel::Public,
                3,
            );
            store
                .add(&entry_a, &make_embedding(i as f32))
                .await
                .unwrap();
        }

        // Bot A at cap
        let result = store
            .add(
                &make_shared_entry(bot_a, "BotA", "Over cap", TrustLevel::Public, 3),
                &make_embedding(99.0),
            )
            .await;
        assert!(result.is_err(), "Bot A should be at cap");

        // Bot B should still be able to add
        let entry_b = make_shared_entry(bot_b, "BotB", "Bot B fact", TrustLevel::Public, 3);
        store
            .add(&entry_b, &make_embedding(50.0))
            .await
            .unwrap();
        assert_eq!(store.count_by_author(&bot_b).await.unwrap(), 1);
    }

    // --- Provenance Tests ---

    #[tokio::test]
    async fn test_provenance_annotation() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "HelperBot",
            "A helpful fact",
            TrustLevel::Public,
            3,
        );
        let embedding = make_embedding(1.0);
        store.add(&entry, &embedding).await.unwrap();

        let results = store
            .search(&bot_b, &[], &embedding, 10, 0.0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].provenance,
            Some("Written by HelperBot".to_string())
        );
    }

    // --- Search Ranking Tests ---

    #[tokio::test]
    async fn test_search_returns_sorted_by_relevance() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        // Add several memories with different embeddings
        for i in 0..5 {
            let entry = make_shared_entry(
                bot_a,
                "BotA",
                &format!("Shared fact {i}"),
                TrustLevel::Public,
                (i + 1) as u8,
            );
            store.add(&entry, &make_embedding(i as f32)).await.unwrap();
        }

        let query = make_embedding(0.0);
        let results = store.search(&bot_b, &[], &query, 3, 0.0).await.unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 3);

        // Results should be sorted by relevance_score descending
        for window in results.windows(2) {
            assert!(
                window[0].relevance_score >= window[1].relevance_score - f32::EPSILON,
                "Results should be sorted by relevance descending"
            );
        }
    }

    #[tokio::test]
    async fn test_search_min_similarity_filter() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let entry =
            make_shared_entry(bot_a, "BotA", "Niche fact", TrustLevel::Public, 3);
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        // Search with a very different embedding and high threshold
        let query = make_embedding(100.0);
        let results = store
            .search(&bot_b, &[], &query, 10, 0.99)
            .await
            .unwrap();

        // All returned results must meet the similarity threshold
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
        let bot_a = Uuid::now_v7();

        store.ensure_shared_table().await.unwrap();

        let query = make_embedding(0.0);
        let results = store
            .search(&bot_a, &[], &query, 10, 0.0)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    // --- Write Hash Tests ---

    #[test]
    fn test_compute_write_hash_deterministic() {
        let bot_id = Uuid::now_v7();
        let entry = SharedMemoryEntry {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            fact: "Test fact".to_string(),
            category: MemoryCategory::Fact,
            importance: 3,
            author_bot_id: bot_id,
            author_bot_name: "TestBot".to_string(),
            trust_level: TrustLevel::Public,
            embedding_model: "bge-small-en-v1.5".to_string(),
            write_hash: String::new(),
            created_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };

        let hash1 = LanceSharedMemoryStore::compute_write_hash(&entry);
        let hash2 = LanceSharedMemoryStore::compute_write_hash(&entry);

        assert_eq!(hash1, hash2, "Hash should be deterministic");
        assert_eq!(hash1.len(), 64, "SHA-256 hex should be 64 characters");
    }

    #[test]
    fn test_compute_write_hash_changes_with_content() {
        let bot_id = Uuid::now_v7();
        let now = Utc::now();

        let entry1 = SharedMemoryEntry {
            id: Uuid::now_v7(),
            fact: "Fact A".to_string(),
            category: MemoryCategory::Fact,
            importance: 3,
            author_bot_id: bot_id,
            author_bot_name: "Bot".to_string(),
            trust_level: TrustLevel::Public,
            embedding_model: "bge-small-en-v1.5".to_string(),
            write_hash: String::new(),
            created_at: now,
        };

        let mut entry2 = entry1.clone();
        entry2.fact = "Fact B".to_string();

        let hash1 = LanceSharedMemoryStore::compute_write_hash(&entry1);
        let hash2 = LanceSharedMemoryStore::compute_write_hash(&entry2);

        assert_ne!(hash1, hash2, "Different content should produce different hashes");
    }

    // --- RecordBatch Roundtrip Test ---

    #[test]
    fn test_record_batch_roundtrip() {
        let bot_id = Uuid::now_v7();
        let mut entry = SharedMemoryEntry {
            id: Uuid::now_v7(),
            fact: "Roundtrip test fact".to_string(),
            category: MemoryCategory::Preference,
            importance: 4,
            author_bot_id: bot_id,
            author_bot_name: "TestBot".to_string(),
            trust_level: TrustLevel::Trusted,
            embedding_model: "bge-small-en-v1.5".to_string(),
            write_hash: String::new(),
            created_at: Utc::now(),
        };
        entry.write_hash = LanceSharedMemoryStore::compute_write_hash(&entry);

        let embedding = make_embedding(42.0);
        let batch = LanceSharedMemoryStore::build_record_batch(&entry, &embedding).unwrap();

        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 11);

        let entries = LanceSharedMemoryStore::record_batch_to_entries(&batch);
        assert_eq!(entries.len(), 1);

        let recovered = &entries[0];
        assert_eq!(recovered.id, entry.id);
        assert_eq!(recovered.fact, entry.fact);
        assert_eq!(recovered.category, entry.category);
        assert_eq!(recovered.importance, entry.importance);
        assert_eq!(recovered.author_bot_id, entry.author_bot_id);
        assert_eq!(recovered.author_bot_name, entry.author_bot_name);
        assert_eq!(recovered.trust_level, entry.trust_level);
        assert_eq!(recovered.embedding_model, entry.embedding_model);
        assert_eq!(recovered.write_hash, entry.write_hash);
    }

    // --- Trust Filter SQL Tests ---

    #[test]
    fn test_build_trust_filter_no_trusted() {
        let bot_id = Uuid::now_v7();
        let filter = LanceSharedMemoryStore::build_trust_filter(&bot_id, &[]);

        assert!(filter.contains("trust_level = 'public'"));
        assert!(filter.contains(&format!("author_bot_id = '{bot_id}'")));
        assert!(!filter.contains("IN ("));
    }

    #[test]
    fn test_build_trust_filter_with_trusted() {
        let reading = Uuid::now_v7();
        let trusted1 = Uuid::now_v7();
        let trusted2 = Uuid::now_v7();

        let filter =
            LanceSharedMemoryStore::build_trust_filter(&reading, &[trusted1, trusted2]);

        assert!(filter.contains("trust_level = 'public'"));
        assert!(filter.contains(&format!("author_bot_id = '{reading}'")));
        assert!(filter.contains("trust_level = 'trusted'"));
        assert!(filter.contains(&format!("'{trusted1}'")));
        assert!(filter.contains(&format!("'{trusted2}'")));
    }

    // --- Integrity After Share/Revoke ---

    #[tokio::test]
    async fn test_integrity_passes_after_share() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "BotA",
            "Will be shared",
            TrustLevel::Private,
            3,
        );
        let entry_id = entry.id;
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        // Share as public (recomputes hash)
        store.share(&entry_id, TrustLevel::Public).await.unwrap();

        // Integrity should still pass with the new hash
        let valid = store.verify_integrity(&entry_id).await.unwrap();
        assert!(valid, "Integrity should pass after share (hash recomputed)");
    }

    #[tokio::test]
    async fn test_integrity_passes_after_revoke() {
        let (store, _tmp) = setup_store().await;
        let bot_a = Uuid::now_v7();

        let entry = make_shared_entry(
            bot_a,
            "BotA",
            "Will be revoked",
            TrustLevel::Public,
            3,
        );
        let entry_id = entry.id;
        store.add(&entry, &make_embedding(1.0)).await.unwrap();

        store.revoke(&entry_id, &bot_a).await.unwrap();

        let valid = store.verify_integrity(&entry_id).await.unwrap();
        assert!(valid, "Integrity should pass after revoke (hash recomputed)");
    }
}
