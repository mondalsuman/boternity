//! SQLite chat repository implementation.
//!
//! Implements `ChatRepository` from `boternity-core` using sqlx with split read/write pools.
//! Follows the same patterns as `SqliteBotRepository`: raw queries, private Row structs,
//! split reader/writer pool usage.

use boternity_core::chat::repository::ChatRepository;
use boternity_types::chat::{ChatMessage, ChatSession, ContextSummary, SessionStatus};
use boternity_types::error::RepositoryError;
use boternity_types::llm::MessageRole;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `ChatRepository`.
pub struct SqliteChatRepository {
    pool: DatabasePool,
}

impl SqliteChatRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Private Row types for SQLite-to-domain mapping
// ---------------------------------------------------------------------------

/// Internal row type for mapping SQLite rows to domain ChatSession.
struct ChatSessionRow {
    id: String,
    bot_id: String,
    title: Option<String>,
    started_at: String,
    ended_at: Option<String>,
    total_input_tokens: i64,
    total_output_tokens: i64,
    message_count: i64,
    model: String,
    status: String,
}

impl ChatSessionRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            bot_id: row.try_get("bot_id")?,
            title: row.try_get("title")?,
            started_at: row.try_get("started_at")?,
            ended_at: row.try_get("ended_at")?,
            total_input_tokens: row.try_get("total_input_tokens")?,
            total_output_tokens: row.try_get("total_output_tokens")?,
            message_count: row.try_get("message_count")?,
            model: row.try_get("model")?,
            status: row.try_get("status")?,
        })
    }

    fn into_session(self) -> Result<ChatSession, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid session id: {e}")))?;
        let bot_id = Uuid::parse_str(&self.bot_id)
            .map_err(|e| RepositoryError::Query(format!("invalid bot_id: {e}")))?;
        let started_at = parse_datetime(&self.started_at)?;
        let ended_at = self
            .ended_at
            .as_deref()
            .map(parse_datetime)
            .transpose()?;
        let status: SessionStatus = self
            .status
            .parse()
            .map_err(|e: String| RepositoryError::Query(e))?;

        Ok(ChatSession {
            id,
            bot_id,
            title: self.title,
            started_at,
            ended_at,
            total_input_tokens: self.total_input_tokens as u32,
            total_output_tokens: self.total_output_tokens as u32,
            message_count: self.message_count as u32,
            model: self.model,
            status,
        })
    }
}

/// Internal row type for mapping SQLite rows to domain ChatMessage.
struct ChatMessageRow {
    id: String,
    session_id: String,
    role: String,
    content: String,
    created_at: String,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    model: Option<String>,
    stop_reason: Option<String>,
    response_ms: Option<i64>,
}

impl ChatMessageRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            role: row.try_get("role")?,
            content: row.try_get("content")?,
            created_at: row.try_get("created_at")?,
            input_tokens: row.try_get("input_tokens")?,
            output_tokens: row.try_get("output_tokens")?,
            model: row.try_get("model")?,
            stop_reason: row.try_get("stop_reason")?,
            response_ms: row.try_get("response_ms")?,
        })
    }

    fn into_message(self) -> Result<ChatMessage, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid message id: {e}")))?;
        let session_id = Uuid::parse_str(&self.session_id)
            .map_err(|e| RepositoryError::Query(format!("invalid session_id: {e}")))?;
        let role: MessageRole = self
            .role
            .parse()
            .map_err(|e: String| RepositoryError::Query(e))?;
        let created_at = parse_datetime(&self.created_at)?;

        Ok(ChatMessage {
            id,
            session_id,
            role,
            content: self.content,
            created_at,
            input_tokens: self.input_tokens.map(|v| v as u32),
            output_tokens: self.output_tokens.map(|v| v as u32),
            model: self.model,
            stop_reason: self.stop_reason,
            response_ms: self.response_ms.map(|v| v as u64),
        })
    }
}

/// Internal row type for mapping SQLite rows to domain ContextSummary.
struct ContextSummaryRow {
    id: String,
    session_id: String,
    summary: String,
    messages_start: i64,
    messages_end: i64,
    token_count: i64,
    created_at: String,
}

impl ContextSummaryRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            summary: row.try_get("summary")?,
            messages_start: row.try_get("messages_start")?,
            messages_end: row.try_get("messages_end")?,
            token_count: row.try_get("token_count")?,
            created_at: row.try_get("created_at")?,
        })
    }

    fn into_summary(self) -> Result<ContextSummary, RepositoryError> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| RepositoryError::Query(format!("invalid summary id: {e}")))?;
        let session_id = Uuid::parse_str(&self.session_id)
            .map_err(|e| RepositoryError::Query(format!("invalid session_id: {e}")))?;
        let created_at = parse_datetime(&self.created_at)?;

        Ok(ContextSummary {
            id,
            session_id,
            summary: self.summary,
            messages_start: self.messages_start as u32,
            messages_end: self.messages_end as u32,
            token_count: self.token_count as u32,
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
// ChatRepository implementation
// ---------------------------------------------------------------------------

impl ChatRepository for SqliteChatRepository {
    async fn create_session(
        &self,
        session: &ChatSession,
    ) -> Result<ChatSession, RepositoryError> {
        sqlx::query(
            r#"INSERT INTO chat_sessions (id, bot_id, title, started_at, ended_at, total_input_tokens, total_output_tokens, message_count, model, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(session.id.to_string())
        .bind(session.bot_id.to_string())
        .bind(&session.title)
        .bind(format_datetime(&session.started_at))
        .bind(session.ended_at.as_ref().map(format_datetime))
        .bind(session.total_input_tokens as i64)
        .bind(session.total_output_tokens as i64)
        .bind(session.message_count as i64)
        .bind(&session.model)
        .bind(session.status.to_string())
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(session.clone())
    }

    async fn get_session(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<ChatSession>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM chat_sessions WHERE id = ?")
            .bind(session_id.to_string())
            .fetch_optional(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let session_row = ChatSessionRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(session_row.into_session()?))
            }
            None => Ok(None),
        }
    }

    async fn update_session(&self, session: &ChatSession) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            r#"UPDATE chat_sessions
               SET title = ?, ended_at = ?, total_input_tokens = ?, total_output_tokens = ?,
                   message_count = ?, status = ?
               WHERE id = ?"#,
        )
        .bind(&session.title)
        .bind(session.ended_at.as_ref().map(format_datetime))
        .bind(session.total_input_tokens as i64)
        .bind(session.total_output_tokens as i64)
        .bind(session.message_count as i64)
        .bind(session.status.to_string())
        .bind(session.id.to_string())
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn list_sessions(
        &self,
        bot_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ChatSession>, RepositoryError> {
        let mut sql =
            String::from("SELECT * FROM chat_sessions WHERE bot_id = ? ORDER BY started_at DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let rows = sqlx::query(&sql)
            .bind(bot_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut sessions = Vec::with_capacity(rows.len());
        for row in &rows {
            let session_row = ChatSessionRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            sessions.push(session_row.into_session()?);
        }

        Ok(sessions)
    }

    async fn delete_session(&self, session_id: &Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM chat_sessions WHERE id = ?")
            .bind(session_id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn get_active_sessions(
        &self,
        bot_id: &Uuid,
    ) -> Result<Vec<ChatSession>, RepositoryError> {
        let rows =
            sqlx::query("SELECT * FROM chat_sessions WHERE bot_id = ? AND status = 'active'")
                .bind(bot_id.to_string())
                .fetch_all(&self.pool.reader)
                .await
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut sessions = Vec::with_capacity(rows.len());
        for row in &rows {
            let session_row = ChatSessionRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            sessions.push(session_row.into_session()?);
        }

        Ok(sessions)
    }

    async fn save_message(&self, message: &ChatMessage) -> Result<(), RepositoryError> {
        // Insert the message
        sqlx::query(
            r#"INSERT INTO chat_messages (id, session_id, role, content, created_at, input_tokens, output_tokens, model, stop_reason, response_ms)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(message.id.to_string())
        .bind(message.session_id.to_string())
        .bind(message.role.to_string())
        .bind(&message.content)
        .bind(format_datetime(&message.created_at))
        .bind(message.input_tokens.map(|v| v as i64))
        .bind(message.output_tokens.map(|v| v as i64))
        .bind(&message.model)
        .bind(&message.stop_reason)
        .bind(message.response_ms.map(|v| v as i64))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        // Increment message_count on the session
        sqlx::query("UPDATE chat_sessions SET message_count = message_count + 1 WHERE id = ?")
            .bind(message.session_id.to_string())
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_messages(
        &self,
        session_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ChatMessage>, RepositoryError> {
        let mut sql = String::from(
            "SELECT * FROM chat_messages WHERE session_id = ? ORDER BY created_at ASC",
        );

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let rows = sqlx::query(&sql)
            .bind(session_id.to_string())
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in &rows {
            let msg_row = ChatMessageRow::from_row(row)
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            messages.push(msg_row.into_message()?);
        }

        Ok(messages)
    }

    async fn get_message_count(&self, session_id: &Uuid) -> Result<u32, RepositoryError> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM chat_messages WHERE session_id = ?")
            .bind(session_id.to_string())
            .fetch_one(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let count: i64 = row
            .try_get("cnt")
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(count as u32)
    }

    async fn save_context_summary(&self, summary: &ContextSummary) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO context_summaries (id, session_id, summary, messages_start, messages_end, token_count, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(summary.id.to_string())
        .bind(summary.session_id.to_string())
        .bind(&summary.summary)
        .bind(summary.messages_start as i64)
        .bind(summary.messages_end as i64)
        .bind(summary.token_count as i64)
        .bind(format_datetime(&summary.created_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_latest_summary(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<ContextSummary>, RepositoryError> {
        let row = sqlx::query(
            "SELECT * FROM context_summaries WHERE session_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let summary_row = ContextSummaryRow::from_row(&row)
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                Ok(Some(summary_row.into_summary()?))
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
        // Leak tempdir so it lives for the test
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn make_session(bot_id: Uuid) -> ChatSession {
        ChatSession {
            id: Uuid::now_v7(),
            bot_id,
            title: None,
            started_at: Utc::now(),
            ended_at: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            message_count: 0,
            model: "claude-sonnet-4-20250514".to_string(),
            status: SessionStatus::Active,
        }
    }

    fn make_message(session_id: Uuid, role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::now_v7(),
            session_id,
            role,
            content: content.to_string(),
            created_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            model: None,
            stop_reason: None,
            response_ms: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        // Create a bot first (needed for FK)
        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("test-bot")
        .bind("Test Bot")
        .bind("A test bot")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        let session = make_session(bot_id);
        let created = repo.create_session(&session).await.unwrap();
        assert_eq!(created.id, session.id);
        assert_eq!(created.status, SessionStatus::Active);

        let found = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(found.id, session.id);
        assert_eq!(found.bot_id, bot_id);
        assert_eq!(found.model, "claude-sonnet-4-20250514");
    }

    #[tokio::test]
    async fn test_update_session() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("update-bot")
        .bind("Update Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        let mut session = make_session(bot_id);
        repo.create_session(&session).await.unwrap();

        session.title = Some("Updated title".to_string());
        session.status = SessionStatus::Completed;
        session.ended_at = Some(Utc::now());
        session.total_input_tokens = 500;
        session.total_output_tokens = 1000;
        repo.update_session(&session).await.unwrap();

        let found = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(found.title.as_deref(), Some("Updated title"));
        assert_eq!(found.status, SessionStatus::Completed);
        assert!(found.ended_at.is_some());
        assert_eq!(found.total_input_tokens, 500);
        assert_eq!(found.total_output_tokens, 1000);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("list-bot")
        .bind("List Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        for _ in 0..3 {
            let session = make_session(bot_id);
            repo.create_session(&session).await.unwrap();
        }

        let all = repo.list_sessions(&bot_id, None, None).await.unwrap();
        assert_eq!(all.len(), 3);

        let page = repo.list_sessions(&bot_id, Some(2), Some(0)).await.unwrap();
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_session_cascades_messages() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("delete-bot")
        .bind("Delete Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        let session = make_session(bot_id);
        repo.create_session(&session).await.unwrap();

        let msg = make_message(session.id, MessageRole::User, "Hello");
        repo.save_message(&msg).await.unwrap();

        repo.delete_session(&session.id).await.unwrap();

        let found = repo.get_session(&session.id).await.unwrap();
        assert!(found.is_none());

        let count = repo.get_message_count(&session.id).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_get_active_sessions() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("active-bot")
        .bind("Active Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        let s1 = make_session(bot_id);
        repo.create_session(&s1).await.unwrap();

        let mut s2 = make_session(bot_id);
        s2.status = SessionStatus::Completed;
        // Create as active first, then update to completed
        repo.create_session(&ChatSession {
            status: SessionStatus::Active,
            ..s2.clone()
        })
        .await
        .unwrap();
        repo.update_session(&s2).await.unwrap();

        let active = repo.get_active_sessions(&bot_id).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, s1.id);
    }

    #[tokio::test]
    async fn test_save_and_get_messages() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("msg-bot")
        .bind("Msg Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        let session = make_session(bot_id);
        repo.create_session(&session).await.unwrap();

        let msg1 = make_message(session.id, MessageRole::User, "Hello");
        let msg2 = ChatMessage {
            input_tokens: Some(50),
            output_tokens: Some(100),
            model: Some("claude-sonnet-4-20250514".to_string()),
            stop_reason: Some("end_turn".to_string()),
            response_ms: Some(1200),
            ..make_message(session.id, MessageRole::Assistant, "Hi there!")
        };

        repo.save_message(&msg1).await.unwrap();
        repo.save_message(&msg2).await.unwrap();

        let messages = repo.get_messages(&session.id, None, None).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].input_tokens, Some(50));
        assert_eq!(messages[1].response_ms, Some(1200));

        let count = repo.get_message_count(&session.id).await.unwrap();
        assert_eq!(count, 2);

        // Verify session message_count was incremented
        let updated_session = repo.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(updated_session.message_count, 2);
    }

    #[tokio::test]
    async fn test_context_summary_crud() {
        let pool = test_pool().await;
        let repo = SqliteChatRepository::new(pool.clone());

        let bot_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO bots (id, slug, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bot_id.to_string())
        .bind("summary-bot")
        .bind("Summary Bot")
        .bind("")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool.writer)
        .await
        .unwrap();

        let session = make_session(bot_id);
        repo.create_session(&session).await.unwrap();

        // No summary yet
        let latest = repo.get_latest_summary(&session.id).await.unwrap();
        assert!(latest.is_none());

        let summary = ContextSummary {
            id: Uuid::now_v7(),
            session_id: session.id,
            summary: "User discussed Rust patterns".to_string(),
            messages_start: 0,
            messages_end: 10,
            token_count: 150,
            created_at: Utc::now(),
        };
        repo.save_context_summary(&summary).await.unwrap();

        let latest = repo.get_latest_summary(&session.id).await.unwrap().unwrap();
        assert_eq!(latest.summary, "User discussed Rust patterns");
        assert_eq!(latest.messages_start, 0);
        assert_eq!(latest.messages_end, 10);
        assert_eq!(latest.token_count, 150);
    }
}
