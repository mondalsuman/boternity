//! Loop guard with depth, rate, and time-window protection for bot-to-bot messaging.
//!
//! Prevents runaway exchange loops by enforcing three layers of protection:
//! - Delegation depth per conversation chain (max hops)
//! - Exchange rate per bot pair (max messages per window)
//! - Time window that resets rate counters

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use uuid::Uuid;

/// Default maximum delegation depth per conversation chain.
const DEFAULT_MAX_DEPTH: u32 = 5;

/// Default maximum exchanges per bot pair per time window.
const DEFAULT_MAX_RATE: u32 = 10;

/// Default time window for rate limiting.
const DEFAULT_WINDOW: Duration = Duration::from_secs(60);

/// Three-layer loop prevention guard for bot-to-bot messaging.
///
/// **Layer 1 -- Depth:** Limits how many hops a conversation chain can take.
/// **Layer 2 -- Rate:** Limits how many messages a bot pair can exchange per window.
/// **Layer 3 -- Time window:** Resets rate counters after the window expires.
pub struct LoopGuard {
    max_depth: u32,
    max_rate: u32,
    window: Duration,
    /// Per-pair exchange counters: (sender, recipient) -> (count, window_start).
    pair_counters: DashMap<(Uuid, Uuid), PairCounter>,
    /// Per-conversation depth tracker: conversation_root_id -> current_depth.
    depth_tracker: DashMap<Uuid, AtomicU64>,
}

/// Rate counter for a bot pair.
struct PairCounter {
    count: u32,
    window_start: Instant,
}

impl LoopGuard {
    /// Create a new loop guard with custom limits.
    pub fn new(max_depth: u32, max_rate: u32, window: Duration) -> Self {
        Self {
            max_depth,
            max_rate,
            window,
            pair_counters: DashMap::new(),
            depth_tracker: DashMap::new(),
        }
    }

    /// Check whether a message from `sender` to `recipient` is allowed.
    ///
    /// Returns `Ok(())` if allowed, or `Err(reason)` if the loop guard rejects it.
    pub fn check(&self, sender: Uuid, recipient: Uuid) -> Result<(), String> {
        self.check_rate(sender, recipient)
    }

    /// Check and increment the rate counter for a bot pair.
    fn check_rate(&self, sender: Uuid, recipient: Uuid) -> Result<(), String> {
        let key = (sender, recipient);
        let mut entry = self.pair_counters.entry(key).or_insert_with(|| PairCounter {
            count: 0,
            window_start: Instant::now(),
        });

        let counter = entry.value_mut();

        // Layer 3: Reset if window expired
        if counter.window_start.elapsed() >= self.window {
            counter.count = 0;
            counter.window_start = Instant::now();
        }

        // Layer 2: Check rate limit
        if counter.count >= self.max_rate {
            return Err(format!(
                "rate limit exceeded: {} -> {} ({} messages in {:?})",
                sender, recipient, counter.count, self.window
            ));
        }

        counter.count += 1;
        Ok(())
    }

    /// Track delegation depth for a conversation chain.
    ///
    /// Call this when a bot delegates a task to another bot. Returns the new
    /// depth, or an error if max depth would be exceeded.
    pub fn track_depth(&self, conversation_id: Uuid) -> Result<u64, String> {
        let entry = self
            .depth_tracker
            .entry(conversation_id)
            .or_insert_with(|| AtomicU64::new(0));

        let new_depth = entry.value().fetch_add(1, Ordering::SeqCst) + 1;

        if new_depth > self.max_depth as u64 {
            // Roll back
            entry.value().fetch_sub(1, Ordering::SeqCst);
            return Err(format!(
                "depth limit exceeded: conversation {} at depth {} (max {})",
                conversation_id, new_depth, self.max_depth
            ));
        }

        Ok(new_depth)
    }

    /// Reset depth tracking for a completed conversation.
    pub fn reset_depth(&self, conversation_id: &Uuid) {
        self.depth_tracker.remove(conversation_id);
    }

    /// Reset all counters (useful for testing).
    pub fn reset_all(&self) {
        self.pair_counters.clear();
        self.depth_tracker.clear();
    }

    /// Get the configured max depth.
    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    /// Get the configured max rate.
    pub fn max_rate(&self) -> u32 {
        self.max_rate
    }
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_DEPTH, DEFAULT_MAX_RATE, DEFAULT_WINDOW)
    }
}

impl std::fmt::Debug for LoopGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopGuard")
            .field("max_depth", &self.max_depth)
            .field("max_rate", &self.max_rate)
            .field("window", &self.window)
            .field("active_pairs", &self.pair_counters.len())
            .field("active_conversations", &self.depth_tracker.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_exchange_allowed() {
        let guard = LoopGuard::default();
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        // Should allow up to max_rate messages
        for _ in 0..guard.max_rate() {
            assert!(guard.check(a, b).is_ok());
        }
    }

    #[test]
    fn rate_limit_enforced() {
        let guard = LoopGuard::new(5, 3, Duration::from_secs(60));
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        // Send 3 (the limit)
        for _ in 0..3 {
            assert!(guard.check(a, b).is_ok());
        }

        // 4th should fail
        let result = guard.check(a, b);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rate limit exceeded"));
    }

    #[test]
    fn rate_limit_is_directional() {
        // A -> B and B -> A are tracked separately
        let guard = LoopGuard::new(5, 2, Duration::from_secs(60));
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        // A -> B: 2 messages (at limit)
        assert!(guard.check(a, b).is_ok());
        assert!(guard.check(a, b).is_ok());
        assert!(guard.check(a, b).is_err()); // 3rd fails

        // B -> A: should still be allowed (separate counter)
        assert!(guard.check(b, a).is_ok());
        assert!(guard.check(b, a).is_ok());
        assert!(guard.check(b, a).is_err()); // 3rd fails
    }

    #[test]
    fn depth_limit_enforced() {
        let guard = LoopGuard::new(3, 10, Duration::from_secs(60));
        let conv = Uuid::now_v7();

        assert_eq!(guard.track_depth(conv).unwrap(), 1);
        assert_eq!(guard.track_depth(conv).unwrap(), 2);
        assert_eq!(guard.track_depth(conv).unwrap(), 3);

        // 4th exceeds max_depth=3
        let result = guard.track_depth(conv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("depth limit exceeded"));
    }

    #[test]
    fn depth_reset_allows_new_chain() {
        let guard = LoopGuard::new(2, 10, Duration::from_secs(60));
        let conv = Uuid::now_v7();

        assert!(guard.track_depth(conv).is_ok());
        assert!(guard.track_depth(conv).is_ok());
        assert!(guard.track_depth(conv).is_err()); // at limit

        // Reset and try again
        guard.reset_depth(&conv);
        assert_eq!(guard.track_depth(conv).unwrap(), 1);
    }

    #[test]
    fn separate_conversations_have_independent_depth() {
        let guard = LoopGuard::new(2, 10, Duration::from_secs(60));
        let conv_a = Uuid::now_v7();
        let conv_b = Uuid::now_v7();

        assert!(guard.track_depth(conv_a).is_ok());
        assert!(guard.track_depth(conv_a).is_ok());
        assert!(guard.track_depth(conv_a).is_err()); // conv_a at limit

        // conv_b is independent
        assert!(guard.track_depth(conv_b).is_ok());
    }

    #[test]
    fn time_window_resets_rate_counters() {
        // Use a very short window so we can test reset
        let guard = LoopGuard::new(5, 2, Duration::from_millis(50));
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        assert!(guard.check(a, b).is_ok());
        assert!(guard.check(a, b).is_ok());
        assert!(guard.check(a, b).is_err()); // at limit

        // Wait for window to expire
        std::thread::sleep(Duration::from_millis(60));

        // Should be allowed again after window reset
        assert!(guard.check(a, b).is_ok());
    }

    #[test]
    fn reset_all_clears_everything() {
        let guard = LoopGuard::new(5, 2, Duration::from_secs(60));
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let conv = Uuid::now_v7();

        // Fill up rate and depth
        guard.check(a, b).unwrap();
        guard.check(a, b).unwrap();
        guard.track_depth(conv).unwrap();

        guard.reset_all();

        // Everything should be fresh
        assert!(guard.check(a, b).is_ok());
        assert_eq!(guard.track_depth(conv).unwrap(), 1);
    }

    #[test]
    fn default_values() {
        let guard = LoopGuard::default();
        assert_eq!(guard.max_depth(), 5);
        assert_eq!(guard.max_rate(), 10);
    }

    #[test]
    fn debug_impl() {
        let guard = LoopGuard::default();
        let debug = format!("{guard:?}");
        assert!(debug.contains("LoopGuard"));
        assert!(debug.contains("max_depth"));
        assert!(debug.contains("max_rate"));
    }
}
