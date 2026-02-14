//! SQLite-backed audit log for skill invocations.
//!
//! Every skill execution is recorded with capabilities used, duration,
//! success/failure, and resource consumption metrics. Input and output
//! are stored as SHA-256 hashes for privacy.

use boternity_types::skill::{Capability, SkillAuditEntry, TrustTier};
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed audit log for skill invocations.
///
/// Provides append-only logging and query methods for security auditing.
pub struct SqliteSkillAuditLog {
    pool: DatabasePool,
}

impl SqliteSkillAuditLog {
    /// Create a new audit log backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Log a skill invocation to the audit trail.
    pub async fn log_invocation(&self, entry: &SkillAuditEntry) -> anyhow::Result<()> {
        let capabilities_json = serde_json::to_string(&entry.capabilities_used)?;

        sqlx::query(
            r#"INSERT INTO skill_audit_log
               (invocation_id, skill_name, skill_version, trust_tier,
                capabilities_used, input_hash, output_hash, fuel_consumed,
                memory_peak_bytes, duration_ms, success, error, timestamp, bot_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(entry.invocation_id.to_string())
        .bind(&entry.skill_name)
        .bind(&entry.skill_version)
        .bind(entry.trust_tier.to_string())
        .bind(&capabilities_json)
        .bind(&entry.input_hash)
        .bind(&entry.output_hash)
        .bind(entry.fuel_consumed.map(|f| f as i64))
        .bind(entry.memory_peak_bytes.map(|m| m as i64))
        .bind(entry.duration_ms as i64)
        .bind(entry.success)
        .bind(&entry.error)
        .bind(entry.timestamp.to_rfc3339())
        .bind(entry.bot_id.to_string())
        .execute(&self.pool.writer)
        .await?;

        Ok(())
    }

    /// Retrieve invocations for a specific skill, ordered by most recent first.
    pub async fn get_invocations_for_skill(
        &self,
        skill_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SkillAuditEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM skill_audit_log WHERE skill_name = ? ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(skill_name)
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await?;

        rows_to_entries(&rows)
    }

    /// Retrieve invocations for a specific bot, ordered by most recent first.
    pub async fn get_invocations_for_bot(
        &self,
        bot_id: &Uuid,
        limit: usize,
    ) -> anyhow::Result<Vec<SkillAuditEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM skill_audit_log WHERE bot_id = ? ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(bot_id.to_string())
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await?;

        rows_to_entries(&rows)
    }

    /// Count total invocations for a specific skill.
    pub async fn count_invocations(&self, skill_name: &str) -> anyhow::Result<u64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM skill_audit_log WHERE skill_name = ?")
                .bind(skill_name)
                .fetch_one(&self.pool.reader)
                .await?;

        Ok(row.0 as u64)
    }
}

// ---------------------------------------------------------------------------
// Private row mapping
// ---------------------------------------------------------------------------

struct SkillAuditRow {
    invocation_id: String,
    skill_name: String,
    skill_version: String,
    trust_tier: String,
    capabilities_used: String,
    input_hash: String,
    output_hash: String,
    fuel_consumed: Option<i64>,
    memory_peak_bytes: Option<i64>,
    duration_ms: i64,
    success: bool,
    error: Option<String>,
    timestamp: String,
    bot_id: String,
}

impl SkillAuditRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            invocation_id: row.try_get("invocation_id")?,
            skill_name: row.try_get("skill_name")?,
            skill_version: row.try_get("skill_version")?,
            trust_tier: row.try_get("trust_tier")?,
            capabilities_used: row.try_get("capabilities_used")?,
            input_hash: row.try_get("input_hash")?,
            output_hash: row.try_get("output_hash")?,
            fuel_consumed: row.try_get("fuel_consumed")?,
            memory_peak_bytes: row.try_get("memory_peak_bytes")?,
            duration_ms: row.try_get("duration_ms")?,
            success: row.try_get("success")?,
            error: row.try_get("error")?,
            timestamp: row.try_get("timestamp")?,
            bot_id: row.try_get("bot_id")?,
        })
    }

    fn into_entry(self) -> anyhow::Result<SkillAuditEntry> {
        let invocation_id = Uuid::parse_str(&self.invocation_id)?;
        let bot_id = Uuid::parse_str(&self.bot_id)?;
        let trust_tier = match self.trust_tier.as_str() {
            "local" => TrustTier::Local,
            "verified" => TrustTier::Verified,
            _ => TrustTier::Untrusted,
        };
        let capabilities_used: Vec<Capability> =
            serde_json::from_str(&self.capabilities_used)?;
        let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339(&self.timestamp)?
            .with_timezone(&Utc);

        Ok(SkillAuditEntry {
            invocation_id,
            skill_name: self.skill_name,
            skill_version: self.skill_version,
            trust_tier,
            capabilities_used,
            input_hash: self.input_hash,
            output_hash: self.output_hash,
            fuel_consumed: self.fuel_consumed.map(|f| f as u64),
            memory_peak_bytes: self.memory_peak_bytes.map(|m| m as usize),
            duration_ms: self.duration_ms as u64,
            success: self.success,
            error: self.error,
            timestamp,
            bot_id,
        })
    }
}

fn rows_to_entries(rows: &[sqlx::sqlite::SqliteRow]) -> anyhow::Result<Vec<SkillAuditEntry>> {
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let audit_row = SkillAuditRow::from_row(row)?;
        entries.push(audit_row.into_entry()?);
    }
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    fn make_audit_entry(bot_id: Uuid, skill_name: &str) -> SkillAuditEntry {
        SkillAuditEntry {
            invocation_id: Uuid::now_v7(),
            skill_name: skill_name.to_string(),
            skill_version: "1.0.0".to_string(),
            trust_tier: TrustTier::Verified,
            capabilities_used: vec![Capability::HttpGet, Capability::ReadFile],
            input_hash: "abc123".to_string(),
            output_hash: "def456".to_string(),
            fuel_consumed: Some(5000),
            memory_peak_bytes: Some(1024 * 1024),
            duration_ms: 150,
            success: true,
            error: None,
            timestamp: Utc::now(),
            bot_id,
        }
    }

    #[tokio::test]
    async fn log_invocation_and_retrieve() {
        let pool = test_pool().await;
        let audit = SqliteSkillAuditLog::new(pool);

        let bot_id = Uuid::now_v7();
        let entry = make_audit_entry(bot_id, "weather-skill");

        audit.log_invocation(&entry).await.unwrap();

        let results = audit
            .get_invocations_for_skill("weather-skill", 10)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill_name, "weather-skill");
        assert_eq!(results[0].skill_version, "1.0.0");
        assert_eq!(results[0].trust_tier, TrustTier::Verified);
        assert_eq!(results[0].capabilities_used.len(), 2);
        assert_eq!(results[0].fuel_consumed, Some(5000));
        assert_eq!(results[0].memory_peak_bytes, Some(1024 * 1024));
        assert_eq!(results[0].duration_ms, 150);
        assert!(results[0].success);
        assert!(results[0].error.is_none());
    }

    #[tokio::test]
    async fn query_by_bot_id() {
        let pool = test_pool().await;
        let audit = SqliteSkillAuditLog::new(pool);

        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        audit
            .log_invocation(&make_audit_entry(bot_a, "skill-1"))
            .await
            .unwrap();
        audit
            .log_invocation(&make_audit_entry(bot_a, "skill-2"))
            .await
            .unwrap();
        audit
            .log_invocation(&make_audit_entry(bot_b, "skill-1"))
            .await
            .unwrap();

        let results_a = audit.get_invocations_for_bot(&bot_a, 10).await.unwrap();
        assert_eq!(results_a.len(), 2);

        let results_b = audit.get_invocations_for_bot(&bot_b, 10).await.unwrap();
        assert_eq!(results_b.len(), 1);
    }

    #[tokio::test]
    async fn count_returns_correct_number() {
        let pool = test_pool().await;
        let audit = SqliteSkillAuditLog::new(pool);

        let bot_id = Uuid::now_v7();

        for _ in 0..5 {
            audit
                .log_invocation(&make_audit_entry(bot_id, "counter-skill"))
                .await
                .unwrap();
        }

        // Add one for a different skill
        audit
            .log_invocation(&make_audit_entry(bot_id, "other-skill"))
            .await
            .unwrap();

        let count = audit.count_invocations("counter-skill").await.unwrap();
        assert_eq!(count, 5);

        let other_count = audit.count_invocations("other-skill").await.unwrap();
        assert_eq!(other_count, 1);
    }

    #[tokio::test]
    async fn log_failed_invocation() {
        let pool = test_pool().await;
        let audit = SqliteSkillAuditLog::new(pool);

        let bot_id = Uuid::now_v7();
        let mut entry = make_audit_entry(bot_id, "failing-skill");
        entry.success = false;
        entry.error = Some("out of fuel".to_string());
        entry.fuel_consumed = Some(1_000_000);

        audit.log_invocation(&entry).await.unwrap();

        let results = audit
            .get_invocations_for_skill("failing-skill", 10)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert_eq!(results[0].error.as_deref(), Some("out of fuel"));
    }
}
