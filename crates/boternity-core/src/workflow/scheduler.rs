//! Cron scheduler wrapping `tokio-cron-scheduler` for workflow triggers.
//!
//! Provides:
//! - Standard cron expression parsing (6-field with seconds)
//! - Human-readable schedule normalization ("every 5 minutes" -> cron)
//! - Missed-run detection and catch-up on restart
//! - Per-workflow job lifecycle (schedule, unschedule, start, stop)

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use super::definition::WorkflowError;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during scheduling operations.
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    /// Failed to create or manipulate a cron job.
    #[error("scheduler error: {0}")]
    JobError(String),

    /// Invalid cron expression or schedule string.
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),

    /// Workflow not found in the scheduler.
    #[error("workflow {0} not registered in scheduler")]
    WorkflowNotFound(Uuid),
}

impl From<SchedulerError> for WorkflowError {
    fn from(e: SchedulerError) -> Self {
        WorkflowError::ExecutionError(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Human-readable schedule normalization
// ---------------------------------------------------------------------------

/// Normalize a human-readable schedule string to a cron expression.
///
/// Supported patterns (case-insensitive):
/// - "every N seconds"     -> "*/N * * * * *"
/// - "every N minutes"     -> "0 */N * * * *"
/// - "every N hours"       -> "0 0 */N * * *"
/// - "every minute"        -> "0 * * * * *"
/// - "every hour"          -> "0 0 * * * *"
/// - "every day"           -> "0 0 0 * * *"
/// - "every day at HH:MM"  -> "0 MM HH * * *"
/// - "hourly"              -> "0 0 * * * *"
/// - "daily"               -> "0 0 0 * * *"
///
/// If the string is already a valid cron expression (contains spaces and
/// doesn't start with a known keyword), it's returned as-is.
pub fn normalize_schedule(input: &str) -> Result<String, SchedulerError> {
    let trimmed = input.trim();

    // If it looks like a cron expression (5 or 6 space-separated fields),
    // validate and return.
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() == 5 {
        // Standard 5-field cron -- prepend "0" for seconds
        return Ok(format!("0 {trimmed}"));
    }
    if parts.len() == 6 {
        // Already 6-field cron with seconds
        return Ok(trimmed.to_string());
    }

    // Try human-readable patterns
    let lower = trimmed.to_lowercase();

    if lower == "every minute" || lower == "minutely" {
        return Ok("0 * * * * *".to_string());
    }
    if lower == "every hour" || lower == "hourly" {
        return Ok("0 0 * * * *".to_string());
    }
    if lower == "every day" || lower == "daily" {
        return Ok("0 0 0 * * *".to_string());
    }

    // "every N seconds/minutes/hours"
    if let Some(rest) = lower.strip_prefix("every ") {
        // "every day at HH:MM"
        if let Some(at_part) = rest.strip_prefix("day at ") {
            let time_parts: Vec<&str> = at_part.split(':').collect();
            if time_parts.len() == 2 {
                let hour: u32 = time_parts[0]
                    .trim()
                    .parse()
                    .map_err(|_| SchedulerError::InvalidSchedule(input.to_string()))?;
                let minute: u32 = time_parts[1]
                    .trim()
                    .parse()
                    .map_err(|_| SchedulerError::InvalidSchedule(input.to_string()))?;
                if hour < 24 && minute < 60 {
                    return Ok(format!("0 {minute} {hour} * * *"));
                }
            }
            return Err(SchedulerError::InvalidSchedule(input.to_string()));
        }

        // Parse "N unit" patterns
        let words: Vec<&str> = rest.split_whitespace().collect();
        if words.len() == 2 {
            let n: u32 = words[0]
                .parse()
                .map_err(|_| SchedulerError::InvalidSchedule(input.to_string()))?;
            if n == 0 {
                return Err(SchedulerError::InvalidSchedule(
                    "interval must be > 0".to_string(),
                ));
            }
            let unit = words[1].trim_end_matches('s');
            return match unit {
                "second" => Ok(format!("*/{n} * * * * *")),
                "minute" => Ok(format!("0 */{n} * * * *")),
                "hour" => Ok(format!("0 0 */{n} * * *")),
                _ => Err(SchedulerError::InvalidSchedule(input.to_string())),
            };
        }
    }

    Err(SchedulerError::InvalidSchedule(format!(
        "unrecognized schedule format: '{trimmed}'"
    )))
}

// ---------------------------------------------------------------------------
// CronScheduler
// ---------------------------------------------------------------------------

/// Callback type invoked when a cron trigger fires.
pub type CronCallback =
    Arc<dyn Fn(Uuid, DateTime<Utc>) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync>;

/// Tracks a registered cron job for a workflow.
struct ScheduledWorkflow {
    /// The job UUID assigned by tokio-cron-scheduler.
    job_id: Uuid,
    /// The normalized cron expression.
    cron_expr: String,
    /// Timestamp of the last successful fire.
    last_fired: Option<DateTime<Utc>>,
}

/// Cron scheduler that wraps `tokio-cron-scheduler::JobScheduler`.
///
/// Manages the lifecycle of cron-triggered workflows:
/// - Schedules workflows with cron expressions (standard or human-readable)
/// - Provides missed-run detection for catch-up on restart
/// - Supports start/stop lifecycle
pub struct CronScheduler {
    /// The underlying tokio-cron-scheduler instance.
    inner: Arc<RwLock<Option<JobScheduler>>>,
    /// Registered workflows: workflow_id -> job metadata.
    workflows: Arc<RwLock<HashMap<Uuid, ScheduledWorkflow>>>,
}

impl CronScheduler {
    /// Create a new cron scheduler (not yet started).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
            workflows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the scheduler. Must be called before scheduling workflows.
    pub async fn start(&self) -> Result<(), SchedulerError> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|e| SchedulerError::JobError(e.to_string()))?;

        scheduler
            .start()
            .await
            .map_err(|e| SchedulerError::JobError(e.to_string()))?;

        let mut inner = self.inner.write().await;
        *inner = Some(scheduler);

        tracing::info!("cron scheduler started");
        Ok(())
    }

    /// Stop the scheduler and remove all jobs.
    pub async fn stop(&self) -> Result<(), SchedulerError> {
        let mut inner = self.inner.write().await;
        if let Some(mut scheduler) = inner.take() {
            scheduler
                .shutdown()
                .await
                .map_err(|e| SchedulerError::JobError(e.to_string()))?;
            tracing::info!("cron scheduler stopped");
        }
        let mut workflows = self.workflows.write().await;
        workflows.clear();
        Ok(())
    }

    /// Schedule a workflow to run on a cron schedule.
    ///
    /// The `schedule` can be a standard cron expression or a human-readable
    /// string (see `normalize_schedule`). The `callback` is invoked each time
    /// the cron fires.
    pub async fn schedule_workflow(
        &self,
        workflow_id: Uuid,
        schedule: &str,
        callback: CronCallback,
    ) -> Result<(), SchedulerError> {
        let cron_expr = normalize_schedule(schedule)?;

        let inner = self.inner.read().await;
        let scheduler = inner
            .as_ref()
            .ok_or_else(|| SchedulerError::JobError("scheduler not started".to_string()))?;

        let wf_id = workflow_id;
        let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
            let cb = callback.clone();
            Box::pin(async move {
                let now = Utc::now();
                tracing::debug!(%wf_id, %now, "cron trigger fired");
                cb(wf_id, now).await;
            })
        })
        .map_err(|e| SchedulerError::InvalidSchedule(e.to_string()))?;

        let job_id = job.guid();
        scheduler
            .add(job)
            .await
            .map_err(|e| SchedulerError::JobError(e.to_string()))?;

        let mut workflows = self.workflows.write().await;
        workflows.insert(
            workflow_id,
            ScheduledWorkflow {
                job_id,
                cron_expr,
                last_fired: None,
            },
        );

        tracing::info!(%workflow_id, %job_id, "workflow scheduled");
        Ok(())
    }

    /// Remove a workflow from the cron scheduler.
    pub async fn unschedule_workflow(&self, workflow_id: Uuid) -> Result<(), SchedulerError> {
        let mut workflows = self.workflows.write().await;
        let entry = workflows
            .remove(&workflow_id)
            .ok_or(SchedulerError::WorkflowNotFound(workflow_id))?;

        let inner = self.inner.read().await;
        if let Some(scheduler) = inner.as_ref() {
            scheduler
                .remove(&entry.job_id)
                .await
                .map_err(|e| SchedulerError::JobError(e.to_string()))?;
        }

        tracing::info!(%workflow_id, "workflow unscheduled");
        Ok(())
    }

    /// Record that a workflow's cron trigger has fired.
    ///
    /// Call this from the callback to update the last-fired timestamp
    /// (used for missed-run detection).
    pub async fn record_fire(&self, workflow_id: Uuid) {
        let mut workflows = self.workflows.write().await;
        if let Some(entry) = workflows.get_mut(&workflow_id) {
            entry.last_fired = Some(Utc::now());
        }
    }

    /// Check for missed cron runs since `last_known_fire`.
    ///
    /// Returns a list of (workflow_id, Vec<missed_timestamps>) for workflows
    /// whose cron schedule would have fired between `last_known_fire` and now
    /// but were not recorded.
    ///
    /// This is used on restart to catch up workflows that should have run
    /// while the scheduler was down.
    pub fn check_missed_runs(
        &self,
        schedules: &[(Uuid, String, Option<DateTime<Utc>>)],
    ) -> Vec<(Uuid, Vec<DateTime<Utc>>)> {
        let now = Utc::now();
        let mut missed = Vec::new();

        for (workflow_id, schedule, last_fired) in schedules {
            let cron_expr = match normalize_schedule(schedule) {
                Ok(expr) => expr,
                Err(_) => continue,
            };

            // Use croner to compute occurrences
            let cron = match cron_expr.parse::<croner::Cron>() {
                Ok(c) => c,
                Err(_) => continue,
            };

            let from = match last_fired {
                Some(t) => *t,
                None => continue, // No baseline, can't detect misses
            };

            let mut missed_times = Vec::new();
            // Iterate occurrences after `from` up to `now`
            for next in cron.iter_after(from) {
                if next >= now {
                    break;
                }
                missed_times.push(next);
            }

            if !missed_times.is_empty() {
                tracing::warn!(
                    %workflow_id,
                    count = missed_times.len(),
                    "detected missed cron runs"
                );
                missed.push((*workflow_id, missed_times));
            }
        }

        missed
    }

    /// Get the number of registered workflows.
    pub async fn workflow_count(&self) -> usize {
        self.workflows.read().await.len()
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // -------------------------------------------------------------------
    // normalize_schedule
    // -------------------------------------------------------------------

    #[test]
    fn test_normalize_standard_5field_cron() {
        let result = normalize_schedule("*/5 * * * *").unwrap();
        assert_eq!(result, "0 */5 * * * *"); // Prepends seconds
    }

    #[test]
    fn test_normalize_6field_cron_passthrough() {
        let result = normalize_schedule("30 */5 * * * *").unwrap();
        assert_eq!(result, "30 */5 * * * *");
    }

    #[test]
    fn test_normalize_every_5_minutes() {
        let result = normalize_schedule("every 5 minutes").unwrap();
        assert_eq!(result, "0 */5 * * * *");
    }

    #[test]
    fn test_normalize_every_10_seconds() {
        let result = normalize_schedule("every 10 seconds").unwrap();
        assert_eq!(result, "*/10 * * * * *");
    }

    #[test]
    fn test_normalize_every_2_hours() {
        let result = normalize_schedule("every 2 hours").unwrap();
        assert_eq!(result, "0 0 */2 * * *");
    }

    #[test]
    fn test_normalize_every_minute() {
        let result = normalize_schedule("every minute").unwrap();
        assert_eq!(result, "0 * * * * *");
    }

    #[test]
    fn test_normalize_hourly() {
        let result = normalize_schedule("hourly").unwrap();
        assert_eq!(result, "0 0 * * * *");
    }

    #[test]
    fn test_normalize_daily() {
        let result = normalize_schedule("daily").unwrap();
        assert_eq!(result, "0 0 0 * * *");
    }

    #[test]
    fn test_normalize_every_day_at_time() {
        let result = normalize_schedule("every day at 09:30").unwrap();
        assert_eq!(result, "0 30 9 * * *");
    }

    #[test]
    fn test_normalize_every_day_at_midnight() {
        let result = normalize_schedule("every day at 00:00").unwrap();
        assert_eq!(result, "0 0 0 * * *");
    }

    #[test]
    fn test_normalize_invalid_format() {
        let result = normalize_schedule("run whenever");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_zero_interval_rejected() {
        let result = normalize_schedule("every 0 minutes");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_case_insensitive() {
        let result = normalize_schedule("Every 5 Minutes").unwrap();
        assert_eq!(result, "0 */5 * * * *");
    }

    #[test]
    fn test_normalize_singular_unit() {
        // "every 1 minute" should work (singular)
        let result = normalize_schedule("every 1 minute").unwrap();
        assert_eq!(result, "0 */1 * * * *");
    }

    // -------------------------------------------------------------------
    // check_missed_runs
    // -------------------------------------------------------------------

    #[test]
    fn test_check_missed_runs_detects_gaps() {
        let scheduler = CronScheduler::new();
        let wf_id = Uuid::now_v7();

        // Last fired 10 minutes ago, runs every minute
        let last_fired = Utc::now() - Duration::minutes(10);
        let schedules = vec![(wf_id, "every minute".to_string(), Some(last_fired))];

        let missed = scheduler.check_missed_runs(&schedules);
        assert_eq!(missed.len(), 1);
        assert_eq!(missed[0].0, wf_id);
        // Should have detected roughly 9 missed runs (10 min ago, every minute)
        let count = missed[0].1.len();
        assert!(
            count >= 8 && count <= 10,
            "expected 8-10 missed runs, got {count}"
        );
    }

    #[test]
    fn test_check_missed_runs_no_gap() {
        let scheduler = CronScheduler::new();
        let wf_id = Uuid::now_v7();

        // Last fired just now, runs every hour -- no misses
        let last_fired = Utc::now() - Duration::seconds(5);
        let schedules = vec![(wf_id, "every hour".to_string(), Some(last_fired))];

        let missed = scheduler.check_missed_runs(&schedules);
        assert!(missed.is_empty(), "expected no missed runs");
    }

    #[test]
    fn test_check_missed_runs_no_baseline() {
        let scheduler = CronScheduler::new();
        let wf_id = Uuid::now_v7();

        // No last_fired -- can't detect misses
        let schedules = vec![(wf_id, "every minute".to_string(), None)];

        let missed = scheduler.check_missed_runs(&schedules);
        assert!(missed.is_empty());
    }

    #[test]
    fn test_check_missed_runs_invalid_schedule_skipped() {
        let scheduler = CronScheduler::new();
        let wf_id = Uuid::now_v7();

        let last_fired = Utc::now() - Duration::hours(1);
        let schedules = vec![(wf_id, "not a schedule".to_string(), Some(last_fired))];

        let missed = scheduler.check_missed_runs(&schedules);
        assert!(missed.is_empty());
    }

    // -------------------------------------------------------------------
    // CronScheduler lifecycle (async)
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let scheduler = CronScheduler::new();
        scheduler.start().await.unwrap();
        assert_eq!(scheduler.workflow_count().await, 0);
        scheduler.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_scheduler_schedule_and_unschedule() {
        let scheduler = CronScheduler::new();
        scheduler.start().await.unwrap();

        let wf_id = Uuid::now_v7();
        let cb: CronCallback = Arc::new(|_id, _time| Box::pin(async {}));

        scheduler
            .schedule_workflow(wf_id, "every 5 minutes", cb)
            .await
            .unwrap();
        assert_eq!(scheduler.workflow_count().await, 1);

        scheduler.unschedule_workflow(wf_id).await.unwrap();
        assert_eq!(scheduler.workflow_count().await, 0);

        scheduler.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_scheduler_schedule_before_start_fails() {
        let scheduler = CronScheduler::new();
        let wf_id = Uuid::now_v7();
        let cb: CronCallback = Arc::new(|_id, _time| Box::pin(async {}));

        let result = scheduler
            .schedule_workflow(wf_id, "every minute", cb)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_scheduler_unschedule_unknown_fails() {
        let scheduler = CronScheduler::new();
        scheduler.start().await.unwrap();

        let result = scheduler
            .unschedule_workflow(Uuid::now_v7())
            .await;
        assert!(result.is_err());

        scheduler.stop().await.unwrap();
    }
}
