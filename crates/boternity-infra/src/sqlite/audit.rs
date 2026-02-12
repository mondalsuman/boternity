//! SQLite memory audit log implementation.
//!
//! Records memory operations (add, delete, share, revoke, merge) for auditing.
//! Provides query methods for bot-scoped and memory-scoped audit trails.

use boternity_types::error::RepositoryError;
use boternity_types::memory::{AuditAction, MemoryAuditEntry};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed memory audit log.
pub struct SqliteAuditLog {
    pool: DatabasePool,
}

impl SqliteAuditLog {
    /// Create a new audit log backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Log a memory audit event.
    pub async fn log(&self, entry: &MemoryAuditEntry) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO memory_audit_log (id, bot_id, memory_id, action, actor, details, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(entry.id.to_string())
        .bind(entry.bot_id.to_string())
        .bind(entry.memory_id.to_string())
        .bind(entry.action.to_string())
        .bind(&entry.actor)
        .bind(&entry.details)
        .bind(format_datetime(&entry.created_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    /// Get all audit entries for a bot, ordered by most recent first.
    pub async fn get_for_bot(
        &self,
        bot_id: &Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<MemoryAuditEntry>, RepositoryError> {
        let mut sql =
            String::from("SELECT * FROM memory_audit_log WHERE bot_id = ? ORDER BY created_at DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let rows = sqlx::query(&sql)
            .bind(bot_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        rows_to_entries(&rows)
    }

    /// Get all audit entries for a specific memory, ordered by most recent first.
    pub async fn get_for_memory(
        &self,
        memory_id: &Uuid,
    ) -> Result<Vec<MemoryAuditEntry>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM memory_audit_log WHERE memory_id = ? ORDER BY created_at DESC",
        )
        .bind(memory_id.to_string())
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        rows_to_entries(&rows)
    }
}

// ---------------------------------------------------------------------------
// Private Row types
// ---------------------------------------------------------------------------

struct AuditRow {
    id: String,
    bot_id: String,
    memory_id: String,
    action: String,
    actor: String,
    details: Option<String>,
    created_at: String,
}

impl AuditRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            bot_id: row.try_get("bot_id")?,
            memory_id: row.try_get("memory_id")?,
            action: row.try_get("action")?,
            actor: row.try_get("actor")?,
            details: row.try_get("details")?,
            created_at: row.try_get("created_at")?,
        })
    }

    fn into_entry(self) -> Result<MemoryAuditEntry, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid audit id: {e}")))?;
        let bot_id = Uuid::parse_str(&self.bot_id)
            .map_err(|e| RepositoryError::Query(format!("invalid bot_id: {e}")))?;
        let memory_id = Uuid::parse_str(&self.memory_id)
            .map_err(|e| RepositoryError::Query(format!("invalid memory_id: {e}")))?;
        let action: AuditAction = self
            .action
            .parse()
            .map_err(|e: String| RepositoryError::Query(e))?;
        let created_at = parse_datetime(&self.created_at)?;

        Ok(MemoryAuditEntry {
            id,
            bot_id,
            memory_id,
            action,
            actor: self.actor,
            details: self.details,
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

fn rows_to_entries(rows: &[sqlx::sqlite::SqliteRow]) -> Result<Vec<MemoryAuditEntry>, RepositoryError> {
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let audit_row =
            AuditRow::from_row(row).map_err(|e| RepositoryError::Query(e.to_string()))?;
        entries.push(audit_row.into_entry()?);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::pool::DatabasePool;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    async fn setup_bot(pool: &DatabasePool) -> Uuid {
        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind(format!("bot-{}", bot_id))
        .bind("Test Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        bot_id
    }

    fn make_audit(bot_id: Uuid, memory_id: Uuid, action: AuditAction, actor: &str) -> MemoryAuditEntry {
        MemoryAuditEntry {
            id: Uuid::now_v7(),
            bot_id,
            memory_id,
            action,
            actor: actor.to_string(),
            details: None,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_log_and_get_for_bot() {
        let pool = test_pool().await;
        let audit = SqliteAuditLog::new(pool.clone());
        let bot_id = setup_bot(&pool).await;
        let memory_id = Uuid::now_v7();

        let entry = make_audit(bot_id, memory_id, AuditAction::Add, "system");
        audit.log(&entry).await.unwrap();

        let entries = audit.get_for_bot(&bot_id, None).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, AuditAction::Add);
        assert_eq!(entries[0].actor, "system");
    }

    #[tokio::test]
    async fn test_get_for_memory() {
        let pool = test_pool().await;
        let audit = SqliteAuditLog::new(pool.clone());
        let bot_id = setup_bot(&pool).await;
        let memory_id = Uuid::now_v7();

        // Log multiple actions on the same memory
        let add = make_audit(bot_id, memory_id, AuditAction::Add, "system");
        let share = make_audit(bot_id, memory_id, AuditAction::Share, "user");
        audit.log(&add).await.unwrap();
        audit.log(&share).await.unwrap();

        let entries = audit.get_for_memory(&memory_id).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_all_audit_actions() {
        let pool = test_pool().await;
        let audit = SqliteAuditLog::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        for action in [
            AuditAction::Add,
            AuditAction::Delete,
            AuditAction::Share,
            AuditAction::Revoke,
            AuditAction::Merge,
        ] {
            let entry = make_audit(bot_id, Uuid::now_v7(), action, "system");
            audit.log(&entry).await.unwrap();
        }

        let entries = audit.get_for_bot(&bot_id, None).await.unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[tokio::test]
    async fn test_audit_with_details() {
        let pool = test_pool().await;
        let audit = SqliteAuditLog::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let mut entry = make_audit(bot_id, Uuid::now_v7(), AuditAction::Merge, "system");
        entry.details = Some(r#"{"merged_from":"abc","merged_into":"def"}"#.to_string());
        audit.log(&entry).await.unwrap();

        let entries = audit.get_for_bot(&bot_id, None).await.unwrap();
        assert!(entries[0].details.is_some());
        assert!(entries[0].details.as_ref().unwrap().contains("merged_from"));
    }

    #[tokio::test]
    async fn test_get_for_bot_with_limit() {
        let pool = test_pool().await;
        let audit = SqliteAuditLog::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        for _ in 0..5 {
            let entry = make_audit(bot_id, Uuid::now_v7(), AuditAction::Add, "system");
            audit.log(&entry).await.unwrap();
        }

        let entries = audit.get_for_bot(&bot_id, Some(3)).await.unwrap();
        assert_eq!(entries.len(), 3);
    }
}
