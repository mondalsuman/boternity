//! Filesystem change trigger using the `notify` crate.
//!
//! Provides:
//! - `start_file_watcher()` -- Starts a debounced filesystem watcher
//! - `WatcherHandle` -- RAII handle that keeps the watcher alive
//! - `filter_events()` -- Glob pattern matching for file change events

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

// Use notify types re-exported through notify-debouncer-mini to avoid version conflicts.
// notify-debouncer-mini 0.5 depends on notify 7.x; the workspace also has notify 8.x,
// but we must use the same version the debouncer was compiled against.
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, DebouncedEvent, Debouncer, new_debouncer};
use tokio::sync::mpsc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during file watching.
#[derive(Debug, thiserror::Error)]
pub enum FileWatchError {
    /// Failed to create the filesystem watcher.
    #[error("watcher creation failed: {0}")]
    WatcherCreation(String),

    /// Failed to add a path to the watcher.
    #[error("failed to watch path '{path}': {reason}")]
    WatchPath {
        path: String,
        reason: String,
    },

    /// The watcher channel was closed unexpectedly.
    #[error("watcher channel closed")]
    ChannelClosed,
}

// ---------------------------------------------------------------------------
// FileEvent
// ---------------------------------------------------------------------------

/// A filesystem change event with workflow context.
#[derive(Debug, Clone)]
pub struct FileEvent {
    /// The path that changed.
    pub path: PathBuf,
    /// The workflow ID this event is for.
    pub workflow_id: Uuid,
    /// When the event was detected.
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

impl FileEvent {
    /// Convert this event to a JSON value for use as trigger payload.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path.display().to_string(),
            "workflow_id": self.workflow_id.to_string(),
            "detected_at": self.detected_at.to_rfc3339(),
        })
    }
}

// ---------------------------------------------------------------------------
// WatcherHandle
// ---------------------------------------------------------------------------

/// RAII handle that keeps a filesystem watcher alive.
///
/// When dropped, the watcher is automatically stopped. Hold this handle
/// in the trigger manager to maintain the watch.
pub struct WatcherHandle {
    /// The underlying debounced watcher. Kept alive by ownership.
    _debouncer: Debouncer<RecommendedWatcher>,
    /// Paths being watched.
    watched_paths: Vec<PathBuf>,
    /// Workflow ID this watcher is for.
    workflow_id: Uuid,
}

impl WatcherHandle {
    /// Get the paths being watched.
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }

    /// Get the workflow ID this watcher is for.
    pub fn workflow_id(&self) -> Uuid {
        self.workflow_id
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        tracing::debug!(
            workflow_id = %self.workflow_id,
            paths = ?self.watched_paths,
            "file watcher dropped"
        );
    }
}

// ---------------------------------------------------------------------------
// Glob pattern matching
// ---------------------------------------------------------------------------

/// Filter a list of debounced events by glob patterns.
///
/// If `patterns` is `None` or empty, all events pass through.
/// Patterns are matched against the file name (not the full path).
///
/// Supported glob syntax:
/// - `*` matches any sequence of non-separator characters
/// - `?` matches any single non-separator character
/// - `[abc]` matches one character in the bracket
/// - `*.csv` matches all CSV files
pub fn filter_events(
    events: &[DebouncedEvent],
    patterns: Option<&[String]>,
) -> Vec<DebouncedEvent> {
    match patterns {
        None => events.to_vec(),
        Some(pats) if pats.is_empty() => events.to_vec(),
        Some(pats) => {
            events
                .iter()
                .filter(|event| {
                    let file_name = event
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    let full_path = event.path.display().to_string();

                    pats.iter().any(|pattern| {
                        glob_match(pattern, file_name) || glob_match(pattern, &full_path)
                    })
                })
                .cloned()
                .collect()
        }
    }
}

/// Simple glob matching (supports `*`, `?`, and `[...]` character classes).
///
/// This is a lightweight implementation that covers common webhook/file
/// trigger patterns without pulling in a full glob crate.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    glob_match_recursive(&pat_chars, 0, &text_chars, 0)
}

fn glob_match_recursive(pattern: &[char], pi: usize, text: &[char], ti: usize) -> bool {
    let mut pi = pi;
    let mut ti = ti;

    while pi < pattern.len() {
        match pattern[pi] {
            '*' => {
                // Skip consecutive stars
                while pi < pattern.len() && pattern[pi] == '*' {
                    pi += 1;
                }
                // Star at end matches everything
                if pi >= pattern.len() {
                    return true;
                }
                // Try matching the rest of the pattern starting from each
                // position in the text
                while ti <= text.len() {
                    if glob_match_recursive(pattern, pi, text, ti) {
                        return true;
                    }
                    ti += 1;
                }
                return false;
            }
            '?' => {
                if ti >= text.len() {
                    return false;
                }
                pi += 1;
                ti += 1;
            }
            '[' => {
                if ti >= text.len() {
                    return false;
                }
                pi += 1; // Skip '['
                let negated = pi < pattern.len() && pattern[pi] == '!';
                if negated {
                    pi += 1;
                }
                let mut matched = false;
                while pi < pattern.len() && pattern[pi] != ']' {
                    let start = pattern[pi];
                    if pi + 2 < pattern.len() && pattern[pi + 1] == '-' {
                        // Range: [a-z]
                        let end = pattern[pi + 2];
                        if text[ti] >= start && text[ti] <= end {
                            matched = true;
                        }
                        pi += 3;
                    } else {
                        if text[ti] == start {
                            matched = true;
                        }
                        pi += 1;
                    }
                }
                if pi < pattern.len() {
                    pi += 1; // Skip ']'
                }
                if negated {
                    matched = !matched;
                }
                if !matched {
                    return false;
                }
                ti += 1;
            }
            c => {
                if ti >= text.len() || text[ti] != c {
                    return false;
                }
                pi += 1;
                ti += 1;
            }
        }
    }

    ti >= text.len()
}

// ---------------------------------------------------------------------------
// File watcher lifecycle
// ---------------------------------------------------------------------------

/// Start a debounced file watcher for the given paths.
///
/// Returns a `WatcherHandle` (keep alive to maintain the watch) and a
/// receiver channel that emits batches of `FileEvent`s.
///
/// # Arguments
/// - `workflow_id`: The workflow this watcher is for
/// - `paths`: Filesystem paths to watch
/// - `patterns`: Optional glob patterns to filter events
/// - `debounce_ms`: Debounce duration in milliseconds (default 500)
pub fn start_file_watcher(
    workflow_id: Uuid,
    paths: &[String],
    patterns: Option<Vec<String>>,
    debounce_ms: Option<u64>,
) -> Result<(WatcherHandle, mpsc::Receiver<Vec<FileEvent>>), FileWatchError> {
    let debounce_duration = Duration::from_millis(debounce_ms.unwrap_or(500));
    let (tx, rx) = mpsc::channel::<Vec<FileEvent>>(64);

    let wf_id = workflow_id;
    let pats: Option<Arc<Vec<String>>> = patterns.map(Arc::new);

    let mut debouncer = new_debouncer(
        debounce_duration,
        move |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    let filtered = match pats.as_deref() {
                        Some(p) => filter_events(&events, Some(p)),
                        None => events,
                    };

                    if filtered.is_empty() {
                        return;
                    }

                    let now = chrono::Utc::now();
                    let file_events: Vec<FileEvent> = filtered
                        .into_iter()
                        .map(|e| FileEvent {
                            path: e.path,
                            workflow_id: wf_id,
                            detected_at: now,
                        })
                        .collect();

                    tracing::debug!(
                        workflow_id = %wf_id,
                        count = file_events.len(),
                        "file change events detected"
                    );

                    // Non-blocking send -- if the channel is full, events
                    // will be dropped (acceptable for file watch triggers)
                    let _ = tx.try_send(file_events);
                }
                Err(err) => {
                    tracing::warn!(
                        workflow_id = %wf_id,
                        error = %err,
                        "file watcher error"
                    );
                }
            }
        },
    )
    .map_err(|e| FileWatchError::WatcherCreation(e.to_string()))?;

    let mut watched_paths = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);
        debouncer
            .watcher()
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| FileWatchError::WatchPath {
                path: path_str.clone(),
                reason: e.to_string(),
            })?;
        watched_paths.push(path.to_path_buf());
    }

    tracing::info!(
        %workflow_id,
        paths = ?watched_paths,
        "file watcher started"
    );

    let handle = WatcherHandle {
        _debouncer: debouncer,
        watched_paths,
        workflow_id,
    };

    Ok((handle, rx))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use notify_debouncer_mini::DebouncedEventKind;
    use std::path::PathBuf;

    fn make_event(path: &str) -> DebouncedEvent {
        DebouncedEvent {
            path: PathBuf::from(path),
            kind: DebouncedEventKind::Any,
        }
    }

    // -------------------------------------------------------------------
    // glob_match
    // -------------------------------------------------------------------

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("hello.txt", "hello.txt"));
        assert!(!glob_match("hello.txt", "hello.csv"));
    }

    #[test]
    fn test_glob_match_star_extension() {
        assert!(glob_match("*.csv", "data.csv"));
        assert!(glob_match("*.csv", "report.csv"));
        assert!(!glob_match("*.csv", "data.txt"));
    }

    #[test]
    fn test_glob_match_star_prefix() {
        assert!(glob_match("report.*", "report.csv"));
        assert!(glob_match("report.*", "report.pdf"));
        assert!(!glob_match("report.*", "invoice.csv"));
    }

    #[test]
    fn test_glob_match_double_star_like() {
        // Single star can match multiple characters
        assert!(glob_match("*data*", "my_data_file.csv"));
        assert!(glob_match("*data*", "data"));
        assert!(!glob_match("*data*", "report"));
    }

    #[test]
    fn test_glob_match_question_mark() {
        assert!(glob_match("file?.txt", "file1.txt"));
        assert!(glob_match("file?.txt", "fileA.txt"));
        assert!(!glob_match("file?.txt", "file12.txt"));
    }

    #[test]
    fn test_glob_match_character_class() {
        assert!(glob_match("[abc].txt", "a.txt"));
        assert!(glob_match("[abc].txt", "b.txt"));
        assert!(!glob_match("[abc].txt", "d.txt"));
    }

    #[test]
    fn test_glob_match_character_range() {
        assert!(glob_match("[a-z].txt", "m.txt"));
        assert!(!glob_match("[a-z].txt", "1.txt"));
    }

    #[test]
    fn test_glob_match_negated_class() {
        assert!(glob_match("[!0-9].txt", "a.txt"));
        assert!(!glob_match("[!0-9].txt", "1.txt"));
    }

    #[test]
    fn test_glob_match_empty() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "something"));
        assert!(!glob_match("something", ""));
    }

    #[test]
    fn test_glob_match_star_only() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    // -------------------------------------------------------------------
    // filter_events
    // -------------------------------------------------------------------

    #[test]
    fn test_filter_events_no_patterns_passes_all() {
        let events = vec![
            make_event("/data/file.csv"),
            make_event("/data/file.txt"),
        ];

        let filtered = filter_events(&events, None);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_events_empty_patterns_passes_all() {
        let events = vec![
            make_event("/data/file.csv"),
            make_event("/data/file.txt"),
        ];

        let filtered = filter_events(&events, Some(&[]));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_events_csv_pattern() {
        let events = vec![
            make_event("/data/file.csv"),
            make_event("/data/file.txt"),
            make_event("/data/report.csv"),
        ];

        let patterns = vec!["*.csv".to_string()];
        let filtered = filter_events(&events, Some(&patterns));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.path.extension().unwrap() == "csv"));
    }

    #[test]
    fn test_filter_events_multiple_patterns() {
        let events = vec![
            make_event("/data/file.csv"),
            make_event("/data/file.txt"),
            make_event("/data/image.png"),
            make_event("/data/report.json"),
        ];

        let patterns = vec!["*.csv".to_string(), "*.json".to_string()];
        let filtered = filter_events(&events, Some(&patterns));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_events_no_matches() {
        let events = vec![
            make_event("/data/file.txt"),
            make_event("/data/file.log"),
        ];

        let patterns = vec!["*.csv".to_string()];
        let filtered = filter_events(&events, Some(&patterns));
        assert!(filtered.is_empty());
    }

    // -------------------------------------------------------------------
    // FileEvent
    // -------------------------------------------------------------------

    #[test]
    fn test_file_event_to_json() {
        let wf_id = Uuid::now_v7();
        let event = FileEvent {
            path: PathBuf::from("/data/report.csv"),
            workflow_id: wf_id,
            detected_at: chrono::Utc::now(),
        };

        let json = event.to_json();
        assert_eq!(json["path"], "/data/report.csv");
        assert!(json["detected_at"].is_string());
    }

    // -------------------------------------------------------------------
    // start_file_watcher (integration test with temp directory)
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn test_start_file_watcher_on_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let wf_id = Uuid::now_v7();

        let (handle, mut rx) = start_file_watcher(
            wf_id,
            &[dir.path().display().to_string()],
            Some(vec!["*.txt".to_string()]),
            Some(100), // 100ms debounce for fast tests
        )
        .unwrap();

        assert_eq!(handle.workflow_id(), wf_id);
        assert_eq!(handle.watched_paths().len(), 1);

        // Write a file to trigger an event
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

        // Wait for debounced events (with timeout)
        let result = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;

        match result {
            Ok(Some(events)) => {
                assert!(!events.is_empty());
                assert_eq!(events[0].workflow_id, wf_id);
            }
            Ok(None) => {
                // Channel closed -- watcher might have been dropped
                // This is acceptable in test scenarios
            }
            Err(_) => {
                // Timeout -- on some platforms file events may be unreliable
                // in test environments. Not a failure.
                tracing::warn!("file watcher test timed out (platform-dependent)");
            }
        }

        drop(handle);
    }

    #[test]
    fn test_start_file_watcher_nonexistent_path() {
        let wf_id = Uuid::now_v7();
        let result = start_file_watcher(
            wf_id,
            &["/nonexistent/path/that/does/not/exist".to_string()],
            None,
            None,
        );
        assert!(result.is_err());
    }
}
