//! SQLite memory repository implementation.
//!
//! Implements `MemoryRepository` from `boternity-core` using sqlx with split read/write pools.
//! Follows the same patterns as `SqliteBotRepository`: raw queries, private Row structs,
//! split reader/writer pool usage.

use boternity_core::memory::store::MemoryRepository;
use boternity_types::error::RepositoryError;
use boternity_types::memory::{MemoryCategory, MemoryEntry, PendingExtraction};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `MemoryRepository`.
pub struct SqliteMemoryRepository {
    pool: DatabasePool,
}

impl SqliteMemoryRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Private Row types for SQLite-to-domain mapping
// ---------------------------------------------------------------------------

/// Internal row type for mapping SQLite rows to domain MemoryEntry.
struct MemoryEntryRow {
    id: String,
    bot_id: String,
    session_id: String,
    fact: String,
    category: String,
    importance: i64,
    source_message_id: Option<String>,
    superseded_by: Option<String>,
    created_at: String,
    is_manual: i64,
}

impl MemoryEntryRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            bot_id: row.try_get("bot_id")?,
            session_id: row.try_get("session_id")?,
            fact: row.try_get("fact")?,
            category: row.try_get("category")?,
            importance: row.try_get("importance")?,
            source_message_id: row.try_get("source_message_id")?,
            superseded_by: row.try_get("superseded_by")?,
            created_at: row.try_get("created_at")?,
            is_manual: row.try_get("is_manual")?,
        })
    }

    fn into_entry(self) -> Result<MemoryEntry, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid memory id: {e}")))?;
        let bot_id = Uuid::parse_str(&self.bot_id)
            .map_err(|e| RepositoryError::Query(format!("invalid bot_id: {e}")))?;
        let session_id = Uuid::parse_str(&self.session_id)
            .map_err(|e| RepositoryError::Query(format!("invalid session_id: {e}")))?;
        let category: MemoryCategory = self
            .category
            .parse()
            .map_err(|e: String| RepositoryError::Query(e))?;
        let source_message_id = self
            .source_message_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| RepositoryError::Query(format!("invalid source_message_id: {e}")))?;
        let superseded_by = self
            .superseded_by
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| RepositoryError::Query(format!("invalid superseded_by: {e}")))?;
        let created_at = parse_datetime(&self.created_at)?;

        Ok(MemoryEntry {
            id,
            bot_id,
            session_id,
            fact: self.fact,
            category,
            importance: self.importance as u8,
            source_message_id,
            superseded_by,
            created_at,
            is_manual: self.is_manual != 0,
            source_agent_id: None,
        })
    }
}

/// Internal row type for mapping SQLite rows to domain PendingExtraction.
struct PendingExtractionRow {
    id: String,
    session_id: String,
    bot_id: String,
    attempt_count: i64,
    last_attempt_at: Option<String>,
    next_attempt_at: String,
    error_message: Option<String>,
    created_at: String,
}

impl PendingExtractionRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            bot_id: row.try_get("bot_id")?,
            attempt_count: row.try_get("attempt_count")?,
            last_attempt_at: row.try_get("last_attempt_at")?,
            next_attempt_at: row.try_get("next_attempt_at")?,
            error_message: row.try_get("error_message")?,
            created_at: row.try_get("created_at")?,
        })
    }

    fn into_pending(self) -> Result<PendingExtraction, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid pending id: {e}")))?;
        let session_id = Uuid::parse_str(&self.session_id)
            .map_err(|e| RepositoryError::Query(format!("invalid session_id: {e}")))?;
        let bot_id = Uuid::parse_str(&self.bot_id)
            .map_err(|e| RepositoryError::Query(format!("invalid bot_id: {e}")))?;
        let last_attempt_at = self
            .last_attempt_at
            .as_deref()
            .map(parse_datetime)
            .transpose()?;
        let next_attempt_at = parse_datetime(&self.next_attempt_at)?;
        let created_at = parse_datetime(&self.created_at)?;

        Ok(PendingExtraction {
            id,
            session_id,
            bot_id,
            attempt_count: self.attempt_count as u32,
            last_attempt_at,
            next_attempt_at,
            error_message: self.error_message,
            created_at,
        })
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

// ---------------------------------------------------------------------------
// MemoryRepository implementation
// ---------------------------------------------------------------------------

impl MemoryRepository for SqliteMemoryRepository {
    async fn save_memory(&self, entry: &MemoryEntry) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO session_memories (id, bot_id, session_id, fact, category, importance, source_message_id, superseded_by, created_at, is_manual)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(entry.id.to_string())
        .bind(entry.bot_id.to_string())
        .bind(entry.session_id.to_string())
        .bind(&entry.fact)
        .bind(entry.category.to_string())
        .bind(entry.importance as i64)
        .bind(entry.source_message_id.map(|id| id.to_string()))
        .bind(entry.superseded_by.map(|id| id.to_string()))
        .bind(format_datetime(&entry.created_at))
        .bind(if entry.is_manual { 1i64 } else { 0i64 })
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_memories(
        &self,
        bot_id: &Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<MemoryEntry>, RepositoryError> {
        let mut sql = String::from(
            "SELECT * FROM session_memories WHERE bot_id = ? ORDER BY importance DESC, created_at DESC",
        );

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let rows = sqlx::query(&sql)
            .bind(bot_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let entry_row = MemoryEntryRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            entries.push(entry_row.into_entry()?);
        }

        Ok(entries)
    }

    async fn delete_memory(&self, memory_id: &Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM session_memories WHERE id = ?")
            .bind(memory_id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn delete_all_memories(&self, bot_id: &Uuid) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM session_memories WHERE bot_id = ?")
            .bind(bot_id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn get_memories_by_session(
        &self,
        session_id: &Uuid,
    ) -> Result<Vec<MemoryEntry>, RepositoryError> {
        let rows = sqlx::query("SELECT * FROM session_memories WHERE session_id = ?")
            .bind(session_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let entry_row = MemoryEntryRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            entries.push(entry_row.into_entry()?);
        }

        Ok(entries)
    }

    async fn save_pending_extraction(
        &self,
        pending: &PendingExtraction,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO pending_memory_extractions (id, session_id, bot_id, attempt_count, last_attempt_at, next_attempt_at, error_message, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(pending.id.to_string())
        .bind(pending.session_id.to_string())
        .bind(pending.bot_id.to_string())
        .bind(pending.attempt_count as i64)
        .bind(pending.last_attempt_at.as_ref().map(format_datetime))
        .bind(format_datetime(&pending.next_attempt_at))
        .bind(&pending.error_message)
        .bind(format_datetime(&pending.created_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_pending_extractions(
        &self,
        bot_id: &Uuid,
    ) -> Result<Vec<PendingExtraction>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM pending_memory_extractions WHERE bot_id = ? AND attempt_count < 3",
        )
        .bind(bot_id.to_string())
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut pending = Vec::with_capacity(rows.len());
        for row in &rows {
            let pending_row = PendingExtractionRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            pending.push(pending_row.into_pending()?);
        }

        Ok(pending)
    }

    async fn delete_pending_extraction(&self, id: &Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM pending_memory_extractions WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn update_pending_extraction(
        &self,
        pending: &PendingExtraction,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            r#"UPDATE pending_memory_extractions
               SET attempt_count = ?, last_attempt_at = ?, next_attempt_at = ?, error_message = ?
               WHERE id = ?"#,
        )
        .bind(pending.attempt_count as i64)
        .bind(pending.last_attempt_at.as_ref().map(format_datetime))
        .bind(format_datetime(&pending.next_attempt_at))
        .bind(&pending.error_message)
        .bind(pending.id.to_string())
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::pool::DatabasePool;
    use boternity_types::chat::SessionStatus;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        // Leak tempdir so it lives for the test
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    /// Helper to insert prerequisite bot and session for memory tests.
    async fn setup_bot_and_session(pool: &DatabasePool) -> (Uuid, Uuid) {
        let bot_id = Uuid::now_v7();
        let session_id = Uuid::now_v7();

        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind(format!("bot-{}", &bot_id.to_string()[..8]))
        .bind("Test Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO chat_sessions (id, bot_id, started_at, model, status) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(session_id.to_string())
        .bind(bot_id.to_string())
        .bind(Utc::now().to_rfc3339())
        .bind("claude-sonnet-4-20250514")
        .bind(SessionStatus::Active.to_string())
        .execute(&pool.writer)
        .await
        .unwrap();

        (bot_id, session_id)
    }

    fn make_memory(bot_id: Uuid, session_id: Uuid, fact: &str, importance: u8) -> MemoryEntry {
        MemoryEntry {
            id: Uuid::now_v7(),
            bot_id,
            session_id,
            fact: fact.to_string(),
            category: MemoryCategory::Fact,
            importance,
            source_message_id: None,
            superseded_by: None,
            created_at: Utc::now(),
            is_manual: false,
            source_agent_id: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_get_memories() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        let m1 = make_memory(bot_id, session_id, "User likes Rust", 4);
        let m2 = make_memory(bot_id, session_id, "User prefers dark mode", 2);
        let m3 = make_memory(bot_id, session_id, "User is a data engineer", 5);

        repo.save_memory(&m1).await.unwrap();
        repo.save_memory(&m2).await.unwrap();
        repo.save_memory(&m3).await.unwrap();

        // Get all, ordered by importance DESC
        let memories = repo.get_memories(&bot_id, None).await.unwrap();
        assert_eq!(memories.len(), 3);
        assert_eq!(memories[0].importance, 5);
        assert_eq!(memories[1].importance, 4);
        assert_eq!(memories[2].importance, 2);

        // With limit
        let limited = repo.get_memories(&bot_id, Some(2)).await.unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_memory() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        let entry = make_memory(bot_id, session_id, "Delete me", 3);
        repo.save_memory(&entry).await.unwrap();

        repo.delete_memory(&entry.id).await.unwrap();

        let memories = repo.get_memories(&bot_id, None).await.unwrap();
        assert!(memories.is_empty());
    }

    #[tokio::test]
    async fn test_delete_all_memories() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        repo.save_memory(&make_memory(bot_id, session_id, "Fact 1", 3))
            .await
            .unwrap();
        repo.save_memory(&make_memory(bot_id, session_id, "Fact 2", 4))
            .await
            .unwrap();

        let deleted = repo.delete_all_memories(&bot_id).await.unwrap();
        assert_eq!(deleted, 2);

        let memories = repo.get_memories(&bot_id, None).await.unwrap();
        assert!(memories.is_empty());
    }

    #[tokio::test]
    async fn test_get_memories_by_session() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        repo.save_memory(&make_memory(bot_id, session_id, "Session fact", 3))
            .await
            .unwrap();

        let session_memories = repo.get_memories_by_session(&session_id).await.unwrap();
        assert_eq!(session_memories.len(), 1);
        assert_eq!(session_memories[0].fact, "Session fact");

        // Other session should have no memories
        let other = repo.get_memories_by_session(&Uuid::now_v7()).await.unwrap();
        assert!(other.is_empty());
    }

    #[tokio::test]
    async fn test_manual_memory() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        let mut entry = make_memory(bot_id, session_id, "Manual fact", 5);
        entry.is_manual = true;
        entry.category = MemoryCategory::Preference;

        repo.save_memory(&entry).await.unwrap();

        let memories = repo.get_memories(&bot_id, None).await.unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].is_manual);
        assert_eq!(memories[0].category, MemoryCategory::Preference);
    }

    #[tokio::test]
    async fn test_pending_extraction_lifecycle() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        let pending = PendingExtraction {
            id: Uuid::now_v7(),
            session_id,
            bot_id,
            attempt_count: 0,
            last_attempt_at: None,
            next_attempt_at: Utc::now(),
            error_message: None,
            created_at: Utc::now(),
        };

        // Save
        repo.save_pending_extraction(&pending).await.unwrap();

        // Get pending (should find it, attempt_count < 3)
        let pending_list = repo.get_pending_extractions(&bot_id).await.unwrap();
        assert_eq!(pending_list.len(), 1);
        assert_eq!(pending_list[0].attempt_count, 0);

        // Update (simulate failed attempt)
        let updated = PendingExtraction {
            attempt_count: 1,
            last_attempt_at: Some(Utc::now()),
            error_message: Some("rate limited".to_string()),
            ..pending.clone()
        };
        repo.update_pending_extraction(&updated).await.unwrap();

        let pending_list = repo.get_pending_extractions(&bot_id).await.unwrap();
        assert_eq!(pending_list[0].attempt_count, 1);
        assert_eq!(
            pending_list[0].error_message.as_deref(),
            Some("rate limited")
        );

        // Delete
        repo.delete_pending_extraction(&pending.id).await.unwrap();
        let pending_list = repo.get_pending_extractions(&bot_id).await.unwrap();
        assert!(pending_list.is_empty());
    }

    #[tokio::test]
    async fn test_pending_extraction_max_retries() {
        let pool = test_pool().await;
        let repo = SqliteMemoryRepository::new(pool.clone());
        let (bot_id, session_id) = setup_bot_and_session(&pool).await;

        let pending = PendingExtraction {
            id: Uuid::now_v7(),
            session_id,
            bot_id,
            attempt_count: 3, // Already at max retries
            last_attempt_at: Some(Utc::now()),
            next_attempt_at: Utc::now(),
            error_message: Some("final failure".to_string()),
            created_at: Utc::now(),
        };

        repo.save_pending_extraction(&pending).await.unwrap();

        // Should NOT appear in get_pending_extractions (attempt_count >= 3)
        let pending_list = repo.get_pending_extractions(&bot_id).await.unwrap();
        assert!(pending_list.is_empty());
    }
}
