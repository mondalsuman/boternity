//! SQLite bot repository implementation.
//!
//! Implements `BotRepository` from `boternity-core` using sqlx with split read/write pools.

use boternity_core::repository::bot::{BotFilter, BotRepository};
use boternity_core::repository::SortOrder;
use boternity_types::bot::{Bot, BotCategory, BotId, BotStatus};
use boternity_types::error::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::Row;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `BotRepository`.
pub struct SqliteBotRepository {
    pool: DatabasePool,
}

impl SqliteBotRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

/// Internal row type for mapping SQLite rows to domain Bot.
struct BotRow {
    id: String,
    slug: String,
    name: String,
    description: String,
    status: String,
    category: String,
    tags: String,
    user_id: Option<String>,
    conversation_count: i64,
    total_tokens_used: i64,
    version_count: i32,
    created_at: String,
    updated_at: String,
    last_active_at: Option<String>,
}

impl BotRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            slug: row.try_get("slug")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            status: row.try_get("status")?,
            category: row.try_get("category")?,
            tags: row.try_get("tags")?,
            user_id: row.try_get("user_id")?,
            conversation_count: row.try_get("conversation_count")?,
            total_tokens_used: row.try_get("total_tokens_used")?,
            version_count: row.try_get("version_count")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            last_active_at: row.try_get("last_active_at")?,
        })
    }

    fn into_bot(self) -> Result<Bot, RepositoryError> {
        let id = self
            .id
            .parse::<BotId>()
            .map_err(|e| RepositoryError::Query(format!("invalid bot id: {e}")))?;

        let status: BotStatus = self
            .status
            .parse()
            .map_err(|e: String| RepositoryError::Query(e))?;

        let category: BotCategory = self
            .category
            .parse()
            .map_err(|e: String| RepositoryError::Query(e))?;

        let tags: Vec<String> = serde_json::from_str(&self.tags)
            .map_err(|e| RepositoryError::Query(format!("invalid tags JSON: {e}")))?;

        let created_at = parse_datetime(&self.created_at)?;
        let updated_at = parse_datetime(&self.updated_at)?;
        let last_active_at = self
            .last_active_at
            .as_deref()
            .map(parse_datetime)
            .transpose()?;

        Ok(Bot {
            id,
            slug: self.slug,
            name: self.name,
            description: self.description,
            status,
            category,
            tags,
            user_id: self.user_id,
            conversation_count: self.conversation_count,
            total_tokens_used: self.total_tokens_used,
            version_count: self.version_count,
            created_at,
            updated_at,
            last_active_at,
        })
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

impl BotRepository for SqliteBotRepository {
    async fn create(&self, bot: &Bot) -> Result<Bot, RepositoryError> {
        let tags_json =
            serde_json::to_string(&bot.tags).map_err(|e| RepositoryError::Query(e.to_string()))?;

        let result = sqlx::query(
            "INSERT INTO bots (id, slug, name, description, status, category, tags, user_id, conversation_count, total_tokens_used, version_count, created_at, updated_at, last_active_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bot.id.to_string())
        .bind(&bot.slug)
        .bind(&bot.name)
        .bind(&bot.description)
        .bind(bot.status.to_string())
        .bind(bot.category.to_string())
        .bind(&tags_json)
        .bind(&bot.user_id)
        .bind(bot.conversation_count)
        .bind(bot.total_tokens_used)
        .bind(bot.version_count)
        .bind(format_datetime(&bot.created_at))
        .bind(format_datetime(&bot.updated_at))
        .bind(bot.last_active_at.as_ref().map(format_datetime))
        .execute(&self.pool.writer)
        .await;

        match result {
            Ok(_) => Ok(bot.clone()),
            Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE") => {
                Err(RepositoryError::Conflict(format!(
                    "slug '{}' already exists",
                    bot.slug
                )))
            }
            Err(e) => Err(RepositoryError::Query(e.to_string())),
        }
    }

    async fn get_by_id(&self, id: &BotId) -> Result<Option<Bot>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM bots WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let bot_row =
                    BotRow::from_row(&row).map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(bot_row.into_bot()?))
            }
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, slug: &str) -> Result<Option<Bot>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM bots WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let bot_row =
                    BotRow::from_row(&row).map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(bot_row.into_bot()?))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, filter: Option<BotFilter>) -> Result<Vec<Bot>, RepositoryError> {
        let mut sql = String::from("SELECT * FROM bots");
        let mut conditions: Vec<String> = Vec::new();

        let filter = filter.unwrap_or_default();

        if let Some(ref status) = filter.status {
            conditions.push(format!("status = '{}'", status));
        }
        if let Some(ref category) = filter.category {
            conditions.push(format!("category = '{}'", category));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        // Sort
        let sort_field = filter.sort_by.as_deref().unwrap_or("created_at");
        // Whitelist allowed sort fields to prevent SQL injection
        let safe_sort = match sort_field {
            "name" | "slug" | "status" | "category" | "created_at" | "updated_at"
            | "last_active_at" | "conversation_count" | "total_tokens_used" => sort_field,
            _ => "created_at",
        };
        let order = match filter.sort_order.unwrap_or_default() {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };
        sql.push_str(&format!(" ORDER BY {safe_sort} {order}"));

        // Pagination
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut bots = Vec::with_capacity(rows.len());
        for row in &rows {
            let bot_row =
                BotRow::from_row(row).map_err(|e| RepositoryError::Query(e.to_string()))?;
            bots.push(bot_row.into_bot()?);
        }

        Ok(bots)
    }

    async fn update(&self, bot: &Bot) -> Result<Bot, RepositoryError> {
        let tags_json =
            serde_json::to_string(&bot.tags).map_err(|e| RepositoryError::Query(e.to_string()))?;

        let result = sqlx::query(
            "UPDATE bots SET slug = ?, name = ?, description = ?, status = ?, category = ?, tags = ?, user_id = ?, conversation_count = ?, total_tokens_used = ?, version_count = ?, updated_at = ?, last_active_at = ?
             WHERE id = ?",
        )
        .bind(&bot.slug)
        .bind(&bot.name)
        .bind(&bot.description)
        .bind(bot.status.to_string())
        .bind(bot.category.to_string())
        .bind(&tags_json)
        .bind(&bot.user_id)
        .bind(bot.conversation_count)
        .bind(bot.total_tokens_used)
        .bind(bot.version_count)
        .bind(format_datetime(&bot.updated_at))
        .bind(bot.last_active_at.as_ref().map(format_datetime))
        .bind(bot.id.to_string())
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(bot.clone())
    }

    async fn delete(&self, id: &BotId) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM bots WHERE id = ?")
            .bind(id.to_string())
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
    use boternity_types::bot::{slugify, BotCategory, BotStatus};

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        // Leak tempdir so it lives for the test
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
            tags: vec!["test".to_string()],
            user_id: None,
            conversation_count: 0,
            total_tokens_used: 0,
            version_count: 0,
            created_at: now,
            updated_at: now,
            last_active_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_by_id() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);
        let bot = make_bot("Luna");

        let created = repo.create(&bot).await.unwrap();
        assert_eq!(created.name, "Luna");

        let found = repo.get_by_id(&bot.id).await.unwrap().unwrap();
        assert_eq!(found.name, "Luna");
        assert_eq!(found.slug, "luna");
        assert_eq!(found.tags, vec!["test"]);
    }

    #[tokio::test]
    async fn test_get_by_slug() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);
        let bot = make_bot("Research Bot");

        repo.create(&bot).await.unwrap();

        let found = repo.get_by_slug("research-bot").await.unwrap().unwrap();
        assert_eq!(found.name, "Research Bot");
    }

    #[tokio::test]
    async fn test_list_with_filters() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);

        let mut bot1 = make_bot("Alpha");
        bot1.status = BotStatus::Active;
        bot1.category = BotCategory::Research;

        let mut bot2 = make_bot("Beta");
        bot2.status = BotStatus::Disabled;
        bot2.category = BotCategory::Creative;

        let mut bot3 = make_bot("Gamma");
        bot3.status = BotStatus::Active;
        bot3.category = BotCategory::Research;

        repo.create(&bot1).await.unwrap();
        repo.create(&bot2).await.unwrap();
        repo.create(&bot3).await.unwrap();

        // List all
        let all = repo.list(None).await.unwrap();
        assert_eq!(all.len(), 3);

        // Filter by status
        let active = repo
            .list(Some(BotFilter {
                status: Some(BotStatus::Active),
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(active.len(), 2);

        // Filter by category
        let research = repo
            .list(Some(BotFilter {
                category: Some(BotCategory::Research),
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(research.len(), 2);

        // Pagination
        let page = repo
            .list(Some(BotFilter {
                limit: Some(1),
                offset: Some(1),
                sort_by: Some("name".to_string()),
                sort_order: Some(SortOrder::Asc),
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].name, "Beta");
    }

    #[tokio::test]
    async fn test_update_status() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);
        let mut bot = make_bot("Updatable");

        repo.create(&bot).await.unwrap();

        bot.status = BotStatus::Archived;
        bot.updated_at = Utc::now();
        repo.update(&bot).await.unwrap();

        let found = repo.get_by_id(&bot.id).await.unwrap().unwrap();
        assert_eq!(found.status, BotStatus::Archived);
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);
        let bot = make_bot("Deletable");

        repo.create(&bot).await.unwrap();
        repo.delete(&bot.id).await.unwrap();

        let found = repo.get_by_id(&bot.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_slug_conflict() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);
        let bot1 = make_bot("Conflict");
        let mut bot2 = make_bot("Conflict");
        bot2.id = BotId::new(); // Different ID but same slug

        repo.create(&bot1).await.unwrap();
        let err = repo.create(&bot2).await.unwrap_err();
        assert!(matches!(err, RepositoryError::Conflict(_)));
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let pool = test_pool().await;
        let repo = SqliteBotRepository::new(pool);

        let err = repo.delete(&BotId::new()).await.unwrap_err();
        assert!(matches!(err, RepositoryError::NotFound));
    }
}
