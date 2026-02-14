//! SQLite implementation of `BuilderMemoryStore`.
//!
//! Persists builder memory entries in the `builder_memory` table.
//! `chosen_skills` is stored as a JSON array text column and deserialized
//! on read. `purpose_category` is stored as its serde serialization string
//! for flexible SQL querying.

use boternity_core::builder::memory::{BuilderMemoryEntry, BuilderMemoryStore};
use boternity_types::builder::PurposeCategory;
use boternity_types::error::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::sqlite::pool::DatabasePool;

/// SQLite-backed builder memory persistence.
pub struct SqliteBuilderMemoryStore {
    pool: DatabasePool,
}

impl SqliteBuilderMemoryStore {
    /// Create a new memory store backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Query(format!("invalid datetime: {e}")))
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

/// Serialize a `PurposeCategory` to its serde JSON string representation
/// for SQL querying.
fn serialize_category(category: &PurposeCategory) -> Result<String, RepositoryError> {
    serde_json::to_string(category)
        .map_err(|e| RepositoryError::Query(format!("failed to serialize category: {e}")))
}

/// Parse a row into a `BuilderMemoryEntry`.
fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<BuilderMemoryEntry, RepositoryError> {
    let id_str: String = row
        .try_get("id")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let purpose_category: String = row
        .try_get("purpose_category")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let initial_description: String = row
        .try_get("initial_description")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let chosen_tone: Option<String> = row
        .try_get("chosen_tone")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let chosen_model: Option<String> = row
        .try_get("chosen_model")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let chosen_skills_json: String = row
        .try_get("chosen_skills")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let bot_slug: Option<String> = row
        .try_get("bot_slug")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

    let chosen_skills: Vec<String> = serde_json::from_str(&chosen_skills_json)
        .map_err(|e| RepositoryError::Query(format!("invalid chosen_skills JSON: {e}")))?;

    Ok(BuilderMemoryEntry {
        id: Uuid::parse_str(&id_str)
            .map_err(|e| RepositoryError::Query(format!("invalid id: {e}")))?,
        purpose_category,
        initial_description,
        chosen_tone,
        chosen_model,
        chosen_skills,
        bot_slug,
        created_at: parse_datetime(&created_at_str)?,
    })
}

// ---------------------------------------------------------------------------
// BuilderMemoryStore implementation
// ---------------------------------------------------------------------------

impl BuilderMemoryStore for SqliteBuilderMemoryStore {
    async fn record_session(
        &self,
        memory: BuilderMemoryEntry,
    ) -> Result<(), RepositoryError> {
        let skills_json = serde_json::to_string(&memory.chosen_skills)
            .map_err(|e| RepositoryError::Query(format!("failed to serialize skills: {e}")))?;

        sqlx::query(
            r#"INSERT INTO builder_memory (id, purpose_category, initial_description, chosen_tone, chosen_model, chosen_skills, bot_slug, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(memory.id.to_string())
        .bind(&memory.purpose_category)
        .bind(&memory.initial_description)
        .bind(&memory.chosen_tone)
        .bind(&memory.chosen_model)
        .bind(&skills_json)
        .bind(&memory.bot_slug)
        .bind(format_datetime(&memory.created_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn recall_by_category(
        &self,
        category: &PurposeCategory,
        limit: usize,
    ) -> Result<Vec<BuilderMemoryEntry>, RepositoryError> {
        let category_str = serialize_category(category)?;

        let rows = sqlx::query(
            "SELECT * FROM builder_memory WHERE purpose_category = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(&category_str)
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            entries.push(row_to_entry(row)?);
        }

        Ok(entries)
    }

    async fn recall_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<BuilderMemoryEntry>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM builder_memory ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            entries.push(row_to_entry(row)?);
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn make_entry(
        category: &str,
        description: &str,
        tone: Option<&str>,
        model: Option<&str>,
        skills: Vec<&str>,
        slug: Option<&str>,
    ) -> BuilderMemoryEntry {
        BuilderMemoryEntry {
            id: Uuid::now_v7(),
            purpose_category: category.to_string(),
            initial_description: description.to_string(),
            chosen_tone: tone.map(|s| s.to_string()),
            chosen_model: model.map(|s| s.to_string()),
            chosen_skills: skills.into_iter().map(|s| s.to_string()).collect(),
            bot_slug: slug.map(|s| s.to_string()),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_record_and_recall_by_category() {
        let pool = test_pool().await;
        let store = SqliteBuilderMemoryStore::new(pool);

        let coding_cat = serialize_category(&PurposeCategory::Coding).unwrap();
        let creative_cat = serialize_category(&PurposeCategory::Creative).unwrap();

        let entry1 = make_entry(
            &coding_cat,
            "A Rust linter",
            Some("formal"),
            Some("claude-sonnet-4-20250514"),
            vec!["code-review"],
            Some("rust-linter"),
        );
        let entry2 = make_entry(
            &coding_cat,
            "A Python helper",
            Some("casual"),
            Some("claude-sonnet-4-20250514"),
            vec!["code-gen", "testing"],
            Some("py-helper"),
        );
        let entry3 = make_entry(
            &creative_cat,
            "A story writer",
            Some("playful"),
            None,
            vec![],
            None,
        );

        store.record_session(entry1).await.unwrap();
        store.record_session(entry2.clone()).await.unwrap();
        store.record_session(entry3).await.unwrap();

        // Recall coding entries only
        let coding_results = store
            .recall_by_category(&PurposeCategory::Coding, 10)
            .await
            .unwrap();
        assert_eq!(coding_results.len(), 2);

        // Most recent first
        assert_eq!(coding_results[0].initial_description, "A Python helper");
        assert_eq!(coding_results[1].initial_description, "A Rust linter");

        // Verify skills deserialized correctly
        assert_eq!(coding_results[0].chosen_skills, vec!["code-gen", "testing"]);

        // Recall creative entries
        let creative_results = store
            .recall_by_category(&PurposeCategory::Creative, 10)
            .await
            .unwrap();
        assert_eq!(creative_results.len(), 1);
        assert_eq!(creative_results[0].initial_description, "A story writer");
    }

    #[tokio::test]
    async fn test_recall_recent_ordering() {
        let pool = test_pool().await;
        let store = SqliteBuilderMemoryStore::new(pool);

        let coding_cat = serialize_category(&PurposeCategory::Coding).unwrap();
        let creative_cat = serialize_category(&PurposeCategory::Creative).unwrap();

        let entry1 = make_entry(&coding_cat, "First bot", None, None, vec![], None);
        store.record_session(entry1).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let entry2 = make_entry(&creative_cat, "Second bot", None, None, vec![], None);
        store.record_session(entry2).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let entry3 = make_entry(&coding_cat, "Third bot", None, None, vec![], None);
        store.record_session(entry3).await.unwrap();

        // Recall recent with limit
        let recent = store.recall_recent(2).await.unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].initial_description, "Third bot");
        assert_eq!(recent[1].initial_description, "Second bot");

        // Recall all
        let all = store.recall_recent(10).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_recall_empty_category() {
        let pool = test_pool().await;
        let store = SqliteBuilderMemoryStore::new(pool);

        let results = store
            .recall_by_category(&PurposeCategory::Research, 10)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_recall_recent_empty() {
        let pool = test_pool().await;
        let store = SqliteBuilderMemoryStore::new(pool);

        let results = store.recall_recent(10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_optional_fields_persist() {
        let pool = test_pool().await;
        let store = SqliteBuilderMemoryStore::new(pool);

        let coding_cat = serialize_category(&PurposeCategory::Coding).unwrap();

        // Entry with all optional fields as None
        let entry = make_entry(&coding_cat, "Minimal bot", None, None, vec![], None);
        store.record_session(entry.clone()).await.unwrap();

        let results = store.recall_recent(1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].chosen_tone.is_none());
        assert!(results[0].chosen_model.is_none());
        assert!(results[0].chosen_skills.is_empty());
        assert!(results[0].bot_slug.is_none());
    }
}
