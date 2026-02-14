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
