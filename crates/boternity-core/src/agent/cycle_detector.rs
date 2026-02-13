//! Cycle detection for agent task hierarchies.
//!
//! `CycleDetector` tracks normalized task signatures and flags when the same
//! task has been attempted too many times, indicating an infinite loop in the
//! agent hierarchy.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

/// Result of checking a task against the cycle detector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleCheckResult {
    /// Task is new or within the repetition threshold.
    Ok,
    /// The same task signature has been seen too many times.
    CycleDetected { description: String },
}

/// Detects repeated task signatures within an agent tree.
///
/// Tasks are normalized (lowercased, trimmed) and hashed. The detector
/// counts how many times each hash has been registered and reports a
/// cycle when the count exceeds the configured threshold.
///
/// Cloning produces a shared view (backed by `Arc<Mutex<...>>`).
#[derive(Debug, Clone)]
pub struct CycleDetector {
    seen_signatures: Arc<Mutex<HashMap<u64, usize>>>,
    max_similar_tasks: usize,
}

impl CycleDetector {
    /// Create a detector with the default threshold of 3 repetitions.
    pub fn new() -> Self {
        Self::with_threshold(3)
    }

    /// Create a detector with a custom repetition threshold.
    pub fn with_threshold(max: usize) -> Self {
        Self {
            seen_signatures: Arc::new(Mutex::new(HashMap::new())),
            max_similar_tasks: max,
        }
    }

    /// Check whether the task at the given depth is a cycle, and register it.
    ///
    /// The task string is normalized (trimmed, lowercased) before hashing.
    /// Returns `CycleDetected` if the same signature has been seen more than
    /// `max_similar_tasks` times.
    pub fn check_and_register(&self, task: &str, _depth: u8) -> CycleCheckResult {
        let normalized = task.trim().to_lowercase();
        let hash = {
            let mut hasher = DefaultHasher::new();
            normalized.hash(&mut hasher);
            hasher.finish()
        };

        let mut map = self.seen_signatures.lock().expect("cycle detector lock poisoned");
        let count = map.entry(hash).or_insert(0);
        *count += 1;

        if *count > self.max_similar_tasks {
            CycleCheckResult::CycleDetected {
                description: format!(
                    "Task '{}' has been attempted {} times (threshold: {})",
                    normalized, *count, self.max_similar_tasks
                ),
            }
        } else {
            CycleCheckResult::Ok
        }
    }
}

impl Default for CycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_occurrence_returns_ok() {
        let detector = CycleDetector::new();
        assert_eq!(detector.check_and_register("Research topic X", 0), CycleCheckResult::Ok);
    }

    #[test]
    fn repeated_tasks_trigger_cycle_detected() {
        let detector = CycleDetector::with_threshold(2);
        assert_eq!(detector.check_and_register("do the thing", 0), CycleCheckResult::Ok);
        assert_eq!(detector.check_and_register("do the thing", 1), CycleCheckResult::Ok);
        // Third time exceeds threshold of 2
        let result = detector.check_and_register("do the thing", 2);
        assert!(matches!(result, CycleCheckResult::CycleDetected { .. }));
    }

    #[test]
    fn different_tasks_dont_interfere() {
        let detector = CycleDetector::with_threshold(1);
        assert_eq!(detector.check_and_register("task A", 0), CycleCheckResult::Ok);
        assert_eq!(detector.check_and_register("task B", 0), CycleCheckResult::Ok);
        assert_eq!(detector.check_and_register("task C", 0), CycleCheckResult::Ok);
    }

    #[test]
    fn normalized_whitespace_and_case() {
        let detector = CycleDetector::with_threshold(1);
        assert_eq!(detector.check_and_register("  Research Topic  ", 0), CycleCheckResult::Ok);
        // Same task with different casing/whitespace should match
        let result = detector.check_and_register("research topic", 1);
        assert!(matches!(result, CycleCheckResult::CycleDetected { .. }));
    }

    #[test]
    fn default_threshold_is_3() {
        let detector = CycleDetector::new();
        assert_eq!(detector.check_and_register("t", 0), CycleCheckResult::Ok);
        assert_eq!(detector.check_and_register("t", 0), CycleCheckResult::Ok);
        assert_eq!(detector.check_and_register("t", 0), CycleCheckResult::Ok);
        // Fourth time exceeds threshold of 3
        assert!(matches!(
            detector.check_and_register("t", 0),
            CycleCheckResult::CycleDetected { .. }
        ));
    }

    #[test]
    fn cycle_detected_includes_description() {
        let detector = CycleDetector::with_threshold(1);
        detector.check_and_register("Summarize results", 0);
        let result = detector.check_and_register("summarize results", 1);
        if let CycleCheckResult::CycleDetected { description } = result {
            assert!(description.contains("summarize results"));
            assert!(description.contains("2 times"));
            assert!(description.contains("threshold: 1"));
        } else {
            panic!("expected CycleDetected");
        }
    }

    #[test]
    fn clone_shares_state() {
        let detector = CycleDetector::with_threshold(2);
        let detector2 = detector.clone();
        detector.check_and_register("shared task", 0);
        detector2.check_and_register("shared task", 0);
        // Third via original should detect cycle
        assert!(matches!(
            detector.check_and_register("shared task", 0),
            CycleCheckResult::CycleDetected { .. }
        ));
    }
}
