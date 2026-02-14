//! SQLite implementation of `BuilderDraftStore`.
//!
//! Persists builder drafts in the `builder_drafts` table using INSERT OR REPLACE
//! for upsert semantics. Extracts `initial_description` and `phase` from the
//! serialized `state_json` for lightweight listing without full deserialization.

use boternity_core::builder::draft_store::{BuilderDraft, BuilderDraftSummary, BuilderDraftStore};
use boternity_types::error::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::sqlite::pool::DatabasePool;

/// SQLite-backed builder draft persistence.
pub struct SqliteBuilderDraftStore {
    pool: DatabasePool,
}

impl SqliteBuilderDraftStore {
    /// Create a new draft store backed by the given database pool.
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

/// Extract a string field from a JSON string without full deserialization.
///
/// This is a lightweight extraction for listing purposes -- avoids
/// deserializing the entire `BuilderState` just to get the description
/// or phase.
fn extract_json_field(json: &str, field: &str) -> String {
    // Use serde_json::Value for correctness (handles escaping, nesting, etc.)
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
        if let Some(s) = value.get(field).and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// BuilderDraftStore implementation
// ---------------------------------------------------------------------------

impl BuilderDraftStore for SqliteBuilderDraftStore {
    async fn save_draft(&self, draft: BuilderDraft) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO builder_drafts (session_id, state_json, schema_version, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(draft.session_id.to_string())
        .bind(&draft.state_json)
        .bind(draft.schema_version as i64)
        .bind(format_datetime(&draft.created_at))
        .bind(format_datetime(&draft.updated_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn load_draft(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<BuilderDraft>, RepositoryError> {
        let row = sqlx::query(
            "SELECT session_id, state_json, schema_version, created_at, updated_at FROM builder_drafts WHERE session_id = ?",
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let sid: String = row
                    .try_get("session_id")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                let state_json: String = row
                    .try_get("state_json")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                let schema_version: i64 = row
                    .try_get("schema_version")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                let created_at_str: String = row
                    .try_get("created_at")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                let updated_at_str: String = row
                    .try_get("updated_at")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;

                Ok(Some(BuilderDraft {
                    session_id: Uuid::parse_str(&sid)
                        .map_err(|e| RepositoryError::Query(format!("invalid session_id: {e}")))?,
                    state_json,
                    schema_version: schema_version as u32,
                    created_at: parse_datetime(&created_at_str)?,
                    updated_at: parse_datetime(&updated_at_str)?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_drafts(&self) -> Result<Vec<BuilderDraftSummary>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT session_id, state_json, updated_at FROM builder_drafts ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut summaries = Vec::with_capacity(rows.len());
        for row in &rows {
            let sid: String = row
                .try_get("session_id")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let state_json: String = row
                .try_get("state_json")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let updated_at_str: String = row
                .try_get("updated_at")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

            summaries.push(BuilderDraftSummary {
                session_id: Uuid::parse_str(&sid)
                    .map_err(|e| RepositoryError::Query(format!("invalid session_id: {e}")))?,
                initial_description: extract_json_field(&state_json, "initial_description"),
                phase: extract_json_field(&state_json, "phase"),
                updated_at: parse_datetime(&updated_at_str)?,
            });
        }

        Ok(summaries)
    }

    async fn delete_draft(&self, session_id: &Uuid) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM builder_drafts WHERE session_id = ?")
            .bind(session_id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
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

    fn make_draft(session_id: Uuid, description: &str, phase: &str) -> BuilderDraft {
        let state_json = serde_json::json!({
            "session_id": session_id.to_string(),
            "phase": phase,
            "initial_description": description,
            "purpose_category": null,
            "conversation": [],
            "config": {},
            "phase_history": []
        })
        .to_string();

        BuilderDraft {
            session_id,
            state_json,
            schema_version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_load_roundtrip() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        let id = Uuid::now_v7();
        let draft = make_draft(id, "A coding bot", "basics");

        store.save_draft(draft.clone()).await.unwrap();

        let loaded = store.load_draft(&id).await.unwrap().unwrap();
        assert_eq!(loaded.session_id, id);
        assert_eq!(loaded.state_json, draft.state_json);
        assert_eq!(loaded.schema_version, 1);
    }

    #[tokio::test]
    async fn test_save_upserts() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        let id = Uuid::now_v7();
        let draft1 = make_draft(id, "A coding bot", "basics");
        store.save_draft(draft1).await.unwrap();

        let draft2 = make_draft(id, "A coding bot", "personality");
        store.save_draft(draft2.clone()).await.unwrap();

        let loaded = store.load_draft(&id).await.unwrap().unwrap();
        assert_eq!(loaded.state_json, draft2.state_json);
    }

    #[tokio::test]
    async fn test_load_nonexistent_returns_none() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        let result = store.load_draft(&Uuid::now_v7()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_drafts_returns_summaries() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();

        store
            .save_draft(make_draft(id1, "Bot alpha", "basics"))
            .await
            .unwrap();
        // Small delay to ensure different updated_at timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store
            .save_draft(make_draft(id2, "Bot beta", "personality"))
            .await
            .unwrap();

        let summaries = store.list_drafts().await.unwrap();
        assert_eq!(summaries.len(), 2);

        // Most recent first
        assert_eq!(summaries[0].session_id, id2);
        assert_eq!(summaries[0].initial_description, "Bot beta");
        assert_eq!(summaries[0].phase, "personality");

        assert_eq!(summaries[1].session_id, id1);
        assert_eq!(summaries[1].initial_description, "Bot alpha");
        assert_eq!(summaries[1].phase, "basics");
    }

    #[tokio::test]
    async fn test_delete_draft_removes_entry() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        let id = Uuid::now_v7();
        store
            .save_draft(make_draft(id, "Temp bot", "basics"))
            .await
            .unwrap();

        store.delete_draft(&id).await.unwrap();

        let result = store.load_draft(&id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_is_noop() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        // Should not error
        store.delete_draft(&Uuid::now_v7()).await.unwrap();
    }

    #[tokio::test]
    async fn test_schema_version_persists() {
        let pool = test_pool().await;
        let store = SqliteBuilderDraftStore::new(pool);

        let id = Uuid::now_v7();
        let mut draft = make_draft(id, "Test", "basics");
        draft.schema_version = 2;

        store.save_draft(draft).await.unwrap();

        let loaded = store.load_draft(&id).await.unwrap().unwrap();
        assert_eq!(loaded.schema_version, 2);
    }
}
