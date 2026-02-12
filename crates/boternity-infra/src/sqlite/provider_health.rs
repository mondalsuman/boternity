//! SQLite provider health persistence.
//!
//! Persists circuit breaker state across application restarts so that
//! provider health information survives process termination.

use boternity_types::error::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::Row;

use super::pool::DatabasePool;

/// A row from the provider_health table.
///
/// This is a persistence-only struct; runtime health tracking uses
/// `ProviderHealth` from `boternity-core`. This struct stores the
/// serializable subset that can be restored on restart.
#[derive(Debug, Clone)]
pub struct ProviderHealthRow {
    pub name: String,
    pub priority: u32,
    pub circuit_state: String,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub last_latency_ms: Option<u64>,
    pub total_calls: u64,
    pub total_failures: u64,
    pub uptime_since: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// SQLite-backed provider health persistence.
pub struct SqliteProviderHealthStore {
    pool: DatabasePool,
}

impl SqliteProviderHealthStore {
    /// Create a new provider health store backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Save (upsert) a provider's health state.
    pub async fn save(&self, row: &ProviderHealthRow) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO provider_health (name, priority, circuit_state, consecutive_failures, last_error, last_latency_ms, total_calls, total_failures, uptime_since, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (name) DO UPDATE SET
                   priority = excluded.priority,
                   circuit_state = excluded.circuit_state,
                   consecutive_failures = excluded.consecutive_failures,
                   last_error = excluded.last_error,
                   last_latency_ms = excluded.last_latency_ms,
                   total_calls = excluded.total_calls,
                   total_failures = excluded.total_failures,
                   uptime_since = excluded.uptime_since,
                   updated_at = excluded.updated_at"#,
        )
        .bind(&row.name)
        .bind(row.priority as i64)
        .bind(&row.circuit_state)
        .bind(row.consecutive_failures as i64)
        .bind(&row.last_error)
        .bind(row.last_latency_ms.map(|v| v as i64))
        .bind(row.total_calls as i64)
        .bind(row.total_failures as i64)
        .bind(row.uptime_since.map(|dt| format_datetime(&dt)))
        .bind(format_datetime(&row.updated_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    /// Load a single provider's persisted health state.
    pub async fn load(&self, name: &str) -> Result<Option<ProviderHealthRow>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM provider_health WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let health_row = HealthSqlRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(health_row.into_provider_health_row()?))
            }
            None => Ok(None),
        }
    }

    /// Load all persisted provider health states.
    pub async fn load_all(&self) -> Result<Vec<ProviderHealthRow>, RepositoryError> {
        let rows = sqlx::query("SELECT * FROM provider_health ORDER BY priority")
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in &rows {
            let health_row = HealthSqlRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            results.push(health_row.into_provider_health_row()?);
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Private Row types
// ---------------------------------------------------------------------------

struct HealthSqlRow {
    name: String,
    priority: i64,
    circuit_state: String,
    consecutive_failures: i64,
    last_error: Option<String>,
    last_latency_ms: Option<i64>,
    total_calls: i64,
    total_failures: i64,
    uptime_since: Option<String>,
    updated_at: String,
}

impl HealthSqlRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            name: row.try_get("name")?,
            priority: row.try_get("priority")?,
            circuit_state: row.try_get("circuit_state")?,
            consecutive_failures: row.try_get("consecutive_failures")?,
            last_error: row.try_get("last_error")?,
            last_latency_ms: row.try_get("last_latency_ms")?,
            total_calls: row.try_get("total_calls")?,
            total_failures: row.try_get("total_failures")?,
            uptime_since: row.try_get("uptime_since")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn into_provider_health_row(self) -> Result<ProviderHealthRow, RepositoryError> {
        let uptime_since = self
            .uptime_since
            .as_deref()
            .map(parse_datetime)
            .transpose()?;
        let updated_at = parse_datetime(&self.updated_at)?;

        Ok(ProviderHealthRow {
            name: self.name,
            priority: self.priority as u32,
            circuit_state: self.circuit_state,
            consecutive_failures: self.consecutive_failures as u32,
            last_error: self.last_error,
            last_latency_ms: self.last_latency_ms.map(|v| v as u64),
            total_calls: self.total_calls as u64,
            total_failures: self.total_failures as u64,
            uptime_since,
            updated_at,
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

    fn make_health_row(name: &str, priority: u32) -> ProviderHealthRow {
        ProviderHealthRow {
            name: name.to_string(),
            priority,
            circuit_state: "closed".to_string(),
            consecutive_failures: 0,
            last_error: None,
            last_latency_ms: None,
            total_calls: 0,
            total_failures: 0,
            uptime_since: Some(Utc::now()),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let pool = test_pool().await;
        let store = SqliteProviderHealthStore::new(pool);

        let row = make_health_row("anthropic", 0);
        store.save(&row).await.unwrap();

        let loaded = store.load("anthropic").await.unwrap().unwrap();
        assert_eq!(loaded.name, "anthropic");
        assert_eq!(loaded.priority, 0);
        assert_eq!(loaded.circuit_state, "closed");
        assert_eq!(loaded.consecutive_failures, 0);
        assert!(loaded.uptime_since.is_some());
    }

    #[tokio::test]
    async fn test_load_nonexistent_returns_none() {
        let pool = test_pool().await;
        let store = SqliteProviderHealthStore::new(pool);

        let loaded = store.load("missing").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_save_upserts() {
        let pool = test_pool().await;
        let store = SqliteProviderHealthStore::new(pool);

        let mut row = make_health_row("openai", 1);
        store.save(&row).await.unwrap();

        // Simulate circuit opening
        row.circuit_state = "open".to_string();
        row.consecutive_failures = 3;
        row.last_error = Some("timeout".to_string());
        row.total_calls = 100;
        row.total_failures = 5;
        row.uptime_since = None;
        store.save(&row).await.unwrap();

        let loaded = store.load("openai").await.unwrap().unwrap();
        assert_eq!(loaded.circuit_state, "open");
        assert_eq!(loaded.consecutive_failures, 3);
        assert_eq!(loaded.last_error.as_deref(), Some("timeout"));
        assert_eq!(loaded.total_calls, 100);
        assert_eq!(loaded.total_failures, 5);
        assert!(loaded.uptime_since.is_none());
    }

    #[tokio::test]
    async fn test_load_all() {
        let pool = test_pool().await;
        let store = SqliteProviderHealthStore::new(pool);

        store.save(&make_health_row("anthropic", 0)).await.unwrap();
        store.save(&make_health_row("openai", 1)).await.unwrap();
        store.save(&make_health_row("gemini", 2)).await.unwrap();

        let all = store.load_all().await.unwrap();
        assert_eq!(all.len(), 3);
        // Ordered by priority
        assert_eq!(all[0].name, "anthropic");
        assert_eq!(all[1].name, "openai");
        assert_eq!(all[2].name, "gemini");
    }

    #[tokio::test]
    async fn test_load_all_empty() {
        let pool = test_pool().await;
        let store = SqliteProviderHealthStore::new(pool);

        let all = store.load_all().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_latency_tracking() {
        let pool = test_pool().await;
        let store = SqliteProviderHealthStore::new(pool);

        let mut row = make_health_row("anthropic", 0);
        row.last_latency_ms = Some(250);
        store.save(&row).await.unwrap();

        let loaded = store.load("anthropic").await.unwrap().unwrap();
        assert_eq!(loaded.last_latency_ms, Some(250));
    }
}
