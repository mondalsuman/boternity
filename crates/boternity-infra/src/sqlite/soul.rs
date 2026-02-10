//! SQLite soul repository implementation.
//!
//! Implements `SoulRepository` from `boternity-core` using sqlx with split read/write pools.
//! Each soul version is immutable once saved. Version count is tracked on the parent bot.

use boternity_core::repository::soul::SoulRepository;
use boternity_types::bot::BotId;
use boternity_types::error::RepositoryError;
use boternity_types::soul::{Soul, SoulId, SoulVersion};
use chrono::{DateTime, Utc};
use sqlx::Row;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `SoulRepository`.
pub struct SqliteSoulRepository {
    pool: DatabasePool,
}

impl SqliteSoulRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Query(format!("invalid datetime: {e}")))
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

fn row_to_soul(row: &sqlx::sqlite::SqliteRow) -> Result<Soul, RepositoryError> {
    let id_str: String = row
        .try_get("id")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let bot_id_str: String = row
        .try_get("bot_id")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let content: String = row
        .try_get("content")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let hash: String = row
        .try_get("hash")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let version: i32 = row
        .try_get("version")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

    Ok(Soul {
        id: id_str
            .parse::<SoulId>()
            .map_err(|e| RepositoryError::Query(format!("invalid soul id: {e}")))?,
        bot_id: bot_id_str
            .parse::<BotId>()
            .map_err(|e| RepositoryError::Query(format!("invalid bot id: {e}")))?,
        content,
        hash,
        version,
        created_at: parse_datetime(&created_at_str)?,
    })
}

fn row_to_soul_version(row: &sqlx::sqlite::SqliteRow) -> Result<SoulVersion, RepositoryError> {
    let version: i32 = row
        .try_get("version")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let hash: String = row
        .try_get("hash")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let content: String = row
        .try_get("content")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;
    let message: Option<String> = row
        .try_get("message")
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

    Ok(SoulVersion {
        version,
        hash,
        content,
        created_at: parse_datetime(&created_at_str)?,
        message,
    })
}

impl SoulRepository for SqliteSoulRepository {
    async fn save_version(&self, soul: &Soul) -> Result<Soul, RepositoryError> {
        // Use a transaction: INSERT soul_version + UPDATE bots.version_count
        let mut tx = self
            .pool
            .writer
            .begin()
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        sqlx::query(
            "INSERT INTO soul_versions (id, bot_id, content, hash, version, message, created_at)
             VALUES (?, ?, ?, ?, ?, NULL, ?)",
        )
        .bind(soul.id.to_string())
        .bind(soul.bot_id.to_string())
        .bind(&soul.content)
        .bind(&soul.hash)
        .bind(soul.version)
        .bind(format_datetime(&soul.created_at))
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.message().contains("UNIQUE") {
                    return RepositoryError::Conflict(format!(
                        "soul version {} already exists for bot {}",
                        soul.version, soul.bot_id
                    ));
                }
            }
            RepositoryError::Query(e.to_string())
        })?;

        // Update bot's version_count
        sqlx::query("UPDATE bots SET version_count = version_count + 1 WHERE id = ?")
            .bind(soul.bot_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(soul.clone())
    }

    async fn get_current(&self, bot_id: &BotId) -> Result<Option<Soul>, RepositoryError> {
        let row = sqlx::query(
            "SELECT * FROM soul_versions WHERE bot_id = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(bot_id.to_string())
        .fetch_optional(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(row_to_soul(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_version(
        &self,
        bot_id: &BotId,
        version: i32,
    ) -> Result<Option<Soul>, RepositoryError> {
        let row =
            sqlx::query("SELECT * FROM soul_versions WHERE bot_id = ? AND version = ?")
                .bind(bot_id.to_string())
                .bind(version)
                .fetch_optional(&self.pool.reader)
                .await
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(row_to_soul(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_versions(
        &self,
        bot_id: &BotId,
    ) -> Result<Vec<SoulVersion>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT * FROM soul_versions WHERE bot_id = ? ORDER BY version ASC",
        )
        .bind(bot_id.to_string())
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut versions = Vec::with_capacity(rows.len());
        for row in &rows {
            versions.push(row_to_soul_version(row)?);
        }
        Ok(versions)
    }

    async fn get_stored_hash(
        &self,
        bot_id: &BotId,
    ) -> Result<Option<String>, RepositoryError> {
        let row = sqlx::query(
            "SELECT hash FROM soul_versions WHERE bot_id = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(bot_id.to_string())
        .fetch_optional(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let hash: String = row
                    .try_get("hash")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::bot::SqliteBotRepository;
    use crate::sqlite::pool::DatabasePool;
    use boternity_core::repository::bot::BotRepository;
    use boternity_types::bot::{slugify, Bot, BotCategory, BotStatus};

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn make_bot(name: &str) -> Bot {
        let now = Utc::now();
        Bot {
            id: BotId::new(),
            slug: slugify(name),
            name: name.to_string(),
            description: format!("A {name} bot"),
            status: BotStatus::Active,
            category: BotCategory::Assistant,
            tags: vec![],
            user_id: None,
            conversation_count: 0,
            total_tokens_used: 0,
            version_count: 0,
            created_at: now,
            updated_at: now,
            last_active_at: None,
        }
    }

    fn make_soul(bot_id: &BotId, version: i32, content: &str) -> Soul {
        use sha2::{Digest, Sha256};
        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        Soul {
            id: SoulId::new(),
            bot_id: bot_id.clone(),
            content: content.to_string(),
            hash,
            version,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_and_get_current() {
        let pool = test_pool().await;
        let bot_repo = SqliteBotRepository::new(pool.clone());
        let soul_repo = SqliteSoulRepository::new(pool);

        let bot = make_bot("Luna");
        bot_repo.create(&bot).await.unwrap();

        let soul = make_soul(&bot.id, 1, "# Luna\nCurious and empathetic.");
        soul_repo.save_version(&soul).await.unwrap();

        let current = soul_repo.get_current(&bot.id).await.unwrap().unwrap();
        assert_eq!(current.version, 1);
        assert!(current.content.contains("Luna"));

        // Bot version_count should be updated
        let updated_bot = bot_repo.get_by_id(&bot.id).await.unwrap().unwrap();
        assert_eq!(updated_bot.version_count, 1);
    }

    #[tokio::test]
    async fn test_multiple_versions() {
        let pool = test_pool().await;
        let bot_repo = SqliteBotRepository::new(pool.clone());
        let soul_repo = SqliteSoulRepository::new(pool);

        let bot = make_bot("Versioned");
        bot_repo.create(&bot).await.unwrap();

        let soul_v1 = make_soul(&bot.id, 1, "Version 1");
        let soul_v2 = make_soul(&bot.id, 2, "Version 2");
        let soul_v3 = make_soul(&bot.id, 3, "Version 3");

        soul_repo.save_version(&soul_v1).await.unwrap();
        soul_repo.save_version(&soul_v2).await.unwrap();
        soul_repo.save_version(&soul_v3).await.unwrap();

        // Current should be v3
        let current = soul_repo.get_current(&bot.id).await.unwrap().unwrap();
        assert_eq!(current.version, 3);
        assert_eq!(current.content, "Version 3");

        // Get specific version
        let v1 = soul_repo.get_version(&bot.id, 1).await.unwrap().unwrap();
        assert_eq!(v1.content, "Version 1");

        // List versions
        let versions = soul_repo.list_versions(&bot.id).await.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[2].version, 3);

        // Bot version_count should reflect all saves
        let updated_bot = bot_repo.get_by_id(&bot.id).await.unwrap().unwrap();
        assert_eq!(updated_bot.version_count, 3);
    }

    #[tokio::test]
    async fn test_get_stored_hash() {
        let pool = test_pool().await;
        let bot_repo = SqliteBotRepository::new(pool.clone());
        let soul_repo = SqliteSoulRepository::new(pool);

        let bot = make_bot("Hasher");
        bot_repo.create(&bot).await.unwrap();

        // No hash initially
        let hash = soul_repo.get_stored_hash(&bot.id).await.unwrap();
        assert!(hash.is_none());

        let soul = make_soul(&bot.id, 1, "Content to hash");
        soul_repo.save_version(&soul).await.unwrap();

        let hash = soul_repo.get_stored_hash(&bot.id).await.unwrap().unwrap();
        assert_eq!(hash, soul.hash);
    }

    #[tokio::test]
    async fn test_foreign_key_cascade_delete() {
        let pool = test_pool().await;
        let bot_repo = SqliteBotRepository::new(pool.clone());
        let soul_repo = SqliteSoulRepository::new(pool);

        let bot = make_bot("Cascadable");
        bot_repo.create(&bot).await.unwrap();

        let soul = make_soul(&bot.id, 1, "Soul to cascade");
        soul_repo.save_version(&soul).await.unwrap();

        // Verify soul exists
        assert!(soul_repo.get_current(&bot.id).await.unwrap().is_some());

        // Delete the bot - should cascade to soul_versions
        bot_repo.delete(&bot.id).await.unwrap();

        // Soul should be gone
        assert!(soul_repo.get_current(&bot.id).await.unwrap().is_none());
    }
}
