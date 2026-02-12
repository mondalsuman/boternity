//! SQLite key-value store implementation.
//!
//! Implements `KvStore` from `boternity-core` using sqlx with split read/write pools.
//! Values are stored as JSON text and deserialized on read.

use boternity_core::storage::kv_store::KvStore;
use boternity_types::error::RepositoryError;
use boternity_types::storage::KvEntry;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `KvStore`.
pub struct SqliteKvStore {
    pool: DatabasePool,
}

impl SqliteKvStore {
    /// Create a new KV store backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Private Row types for SQLite-to-domain mapping
// ---------------------------------------------------------------------------

struct KvRow {
    bot_id: String,
    key: String,
    value: String,
    created_at: String,
    updated_at: String,
}

impl KvRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            bot_id: row.try_get("bot_id")?,
            key: row.try_get("key")?,
            value: row.try_get("value")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn into_entry(self) -> Result<KvEntry, RepositoryError> {
        let bot_id = Uuid::parse_str(&self.bot_id)
            .map_err(|e| RepositoryError::Query(format!("invalid bot_id: {e}")))?;
        let value: serde_json::Value = serde_json::from_str(&self.value)
            .map_err(|e| RepositoryError::Query(format!("invalid JSON value: {e}")))?;
        let created_at = parse_datetime(&self.created_at)?;
        let updated_at = parse_datetime(&self.updated_at)?;

        Ok(KvEntry {
            bot_id,
            key: self.key,
            value,
            created_at,
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

// ---------------------------------------------------------------------------
// KvStore implementation
// ---------------------------------------------------------------------------

impl KvStore for SqliteKvStore {
    async fn get(
        &self,
        bot_id: &Uuid,
        key: &str,
    ) -> Result<Option<serde_json::Value>, RepositoryError> {
        let row = sqlx::query("SELECT value FROM bot_kv_store WHERE bot_id = ? AND key = ?")
            .bind(bot_id.to_string())
            .bind(key)
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let value_str: String = row
                    .try_get("value")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                let value: serde_json::Value = serde_json::from_str(&value_str)
                    .map_err(|e| RepositoryError::Query(format!("invalid JSON value: {e}")))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set(
        &self,
        bot_id: &Uuid,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), RepositoryError> {
        let now = format_datetime(&Utc::now());
        let value_str = serde_json::to_string(value)
            .map_err(|e| RepositoryError::Query(format!("failed to serialize value: {e}")))?;

        sqlx::query(
            r#"INSERT INTO bot_kv_store (bot_id, key, value, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT (bot_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"#,
        )
        .bind(bot_id.to_string())
        .bind(key)
        .bind(&value_str)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, bot_id: &Uuid, key: &str) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM bot_kv_store WHERE bot_id = ? AND key = ?")
            .bind(bot_id.to_string())
            .bind(key)
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn list_keys(&self, bot_id: &Uuid) -> Result<Vec<String>, RepositoryError> {
        let rows = sqlx::query("SELECT key FROM bot_kv_store WHERE bot_id = ? ORDER BY key")
            .bind(bot_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut keys = Vec::with_capacity(rows.len());
        for row in &rows {
            let key: String = row
                .try_get("key")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            keys.push(key);
        }

        Ok(keys)
    }

    async fn get_entry(
        &self,
        bot_id: &Uuid,
        key: &str,
    ) -> Result<Option<KvEntry>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM bot_kv_store WHERE bot_id = ? AND key = ?")
            .bind(bot_id.to_string())
            .bind(key)
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let kv_row = KvRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(kv_row.into_entry()?))
            }
            None => Ok(None),
        }
    }
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

    #[tokio::test]
    async fn test_set_get_roundtrip() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let value = serde_json::json!({"theme": "dark", "font_size": 14});
        store.set(&bot_id, "settings", &value).await.unwrap();

        let got = store.get(&bot_id, "settings").await.unwrap();
        assert_eq!(got, Some(value));
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_none() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let got = store.get(&bot_id, "missing").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_set_upserts() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        store
            .set(&bot_id, "counter", &serde_json::json!(1))
            .await
            .unwrap();
        store
            .set(&bot_id, "counter", &serde_json::json!(2))
            .await
            .unwrap();

        let got = store.get(&bot_id, "counter").await.unwrap();
        assert_eq!(got, Some(serde_json::json!(2)));
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        store
            .set(&bot_id, "temp", &serde_json::json!("value"))
            .await
            .unwrap();
        store.delete(&bot_id, "temp").await.unwrap();

        let got = store.get(&bot_id, "temp").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_is_noop() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        // Should not error
        store.delete(&bot_id, "nope").await.unwrap();
    }

    #[tokio::test]
    async fn test_list_keys() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        store
            .set(&bot_id, "beta", &serde_json::json!("b"))
            .await
            .unwrap();
        store
            .set(&bot_id, "alpha", &serde_json::json!("a"))
            .await
            .unwrap();
        store
            .set(&bot_id, "gamma", &serde_json::json!("g"))
            .await
            .unwrap();

        let keys = store.list_keys(&bot_id).await.unwrap();
        assert_eq!(keys, vec!["alpha", "beta", "gamma"]);
    }

    #[tokio::test]
    async fn test_list_keys_empty() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let keys = store.list_keys(&bot_id).await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_get_entry() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let value = serde_json::json!({"nested": [1, 2, 3]});
        store.set(&bot_id, "data", &value).await.unwrap();

        let entry = store.get_entry(&bot_id, "data").await.unwrap().unwrap();
        assert_eq!(entry.bot_id, bot_id);
        assert_eq!(entry.key, "data");
        assert_eq!(entry.value, value);
        assert!(entry.created_at <= Utc::now());
        assert!(entry.updated_at <= Utc::now());
    }

    #[tokio::test]
    async fn test_get_entry_nonexistent_returns_none() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        let entry = store.get_entry(&bot_id, "missing").await.unwrap();
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_bot_isolation() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_a = setup_bot(&pool).await;
        let bot_b = setup_bot(&pool).await;

        store
            .set(&bot_a, "name", &serde_json::json!("Alice"))
            .await
            .unwrap();
        store
            .set(&bot_b, "name", &serde_json::json!("Bob"))
            .await
            .unwrap();

        let a_val = store.get(&bot_a, "name").await.unwrap();
        let b_val = store.get(&bot_b, "name").await.unwrap();
        assert_eq!(a_val, Some(serde_json::json!("Alice")));
        assert_eq!(b_val, Some(serde_json::json!("Bob")));

        let a_keys = store.list_keys(&bot_a).await.unwrap();
        assert_eq!(a_keys.len(), 1);
    }

    #[tokio::test]
    async fn test_json_value_types() {
        let pool = test_pool().await;
        let store = SqliteKvStore::new(pool.clone());
        let bot_id = setup_bot(&pool).await;

        // String
        store
            .set(&bot_id, "string", &serde_json::json!("hello"))
            .await
            .unwrap();
        assert_eq!(
            store.get(&bot_id, "string").await.unwrap(),
            Some(serde_json::json!("hello"))
        );

        // Number
        store
            .set(&bot_id, "number", &serde_json::json!(42))
            .await
            .unwrap();
        assert_eq!(
            store.get(&bot_id, "number").await.unwrap(),
            Some(serde_json::json!(42))
        );

        // Boolean
        store
            .set(&bot_id, "bool", &serde_json::json!(true))
            .await
            .unwrap();
        assert_eq!(
            store.get(&bot_id, "bool").await.unwrap(),
            Some(serde_json::json!(true))
        );

        // Null
        store
            .set(&bot_id, "null", &serde_json::json!(null))
            .await
            .unwrap();
        assert_eq!(
            store.get(&bot_id, "null").await.unwrap(),
            Some(serde_json::json!(null))
        );

        // Array
        store
            .set(&bot_id, "array", &serde_json::json!([1, "two", 3]))
            .await
            .unwrap();
        assert_eq!(
            store.get(&bot_id, "array").await.unwrap(),
            Some(serde_json::json!([1, "two", 3]))
        );

        // Nested object
        store
            .set(
                &bot_id,
                "nested",
                &serde_json::json!({"a": {"b": {"c": true}}}),
            )
            .await
            .unwrap();
        assert_eq!(
            store.get(&bot_id, "nested").await.unwrap(),
            Some(serde_json::json!({"a": {"b": {"c": true}}}))
        );
    }
}
