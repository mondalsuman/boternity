//! Token budget tracking for agent request execution.
//!
//! `RequestBudget` provides atomic token counting with a configurable budget limit.
//! It detects threshold crossings (80% warning) and budget exhaustion, ensuring
//! the warning fires exactly once even under concurrent access.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Status returned after adding tokens to the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetStatus {
    /// Under the warning threshold (< 80%).
    Ok,
    /// Just crossed the 80% threshold. Returned exactly once per budget lifetime.
    Warning,
    /// At or over 100% of the budget.
    Exhausted,
}

/// Atomic token budget tracker shared across an agent hierarchy.
///
/// All fields use `Arc` so cloning produces a shared view of the same budget.
/// Token additions are lock-free via `AtomicU32::fetch_add`.
#[derive(Debug, Clone)]
pub struct RequestBudget {
    total_budget: u32,
    tokens_used: Arc<AtomicU32>,
    warning_emitted: Arc<AtomicBool>,
}

impl RequestBudget {
    /// Create a new budget with the given total token limit.
    pub fn new(total_budget: u32) -> Self {
        Self {
            total_budget,
            tokens_used: Arc::new(AtomicU32::new(0)),
            warning_emitted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Atomically add tokens and return the resulting budget status.
    ///
    /// - Returns `BudgetStatus::Exhausted` if the new total meets or exceeds the budget.
    /// - Returns `BudgetStatus::Warning` exactly once when crossing the 80% threshold.
    /// - Returns `BudgetStatus::Ok` otherwise.
    pub fn add_tokens(&self, tokens: u32) -> BudgetStatus {
        let prev = self.tokens_used.fetch_add(tokens, Ordering::SeqCst);
        let new_total = prev.saturating_add(tokens);

        if new_total >= self.total_budget {
            return BudgetStatus::Exhausted;
        }

        let threshold = self.total_budget * 80 / 100;
        if prev < threshold
            && new_total >= threshold
            && self
                .warning_emitted
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            return BudgetStatus::Warning;
        }

        BudgetStatus::Ok
    }

    /// Current number of tokens consumed.
    pub fn tokens_used(&self) -> u32 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    /// The total budget limit.
    pub fn total_budget(&self) -> u32 {
        self.total_budget
    }

    /// Remaining tokens before exhaustion (saturating).
    pub fn remaining(&self) -> u32 {
        self.total_budget
            .saturating_sub(self.tokens_used.load(Ordering::SeqCst))
    }

    /// Percentage of budget consumed (0.0 to 100.0+).
    pub fn percentage(&self) -> f32 {
        if self.total_budget == 0 {
            return 100.0;
        }
        self.tokens_used.load(Ordering::SeqCst) as f32 / self.total_budget as f32 * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_tokens_returns_ok_when_under_budget() {
        let budget = RequestBudget::new(1000);
        assert_eq!(budget.add_tokens(100), BudgetStatus::Ok);
        assert_eq!(budget.add_tokens(200), BudgetStatus::Ok);
        assert_eq!(budget.tokens_used(), 300);
    }

    #[test]
    fn add_tokens_returns_warning_exactly_once_at_80_percent() {
        let budget = RequestBudget::new(1000);
        // Add 750 tokens -> 75%, Ok
        assert_eq!(budget.add_tokens(750), BudgetStatus::Ok);
        // Add 50 tokens -> 800/1000 = 80%, Warning (crosses threshold)
        assert_eq!(budget.add_tokens(50), BudgetStatus::Warning);
        // Add 50 more -> 850/1000 = 85%, should NOT return Warning again
        assert_eq!(budget.add_tokens(50), BudgetStatus::Ok);
    }

    #[test]
    fn add_tokens_returns_exhausted_at_or_over_budget() {
        let budget = RequestBudget::new(1000);
        // 500 tokens -> 50%, Ok
        assert_eq!(budget.add_tokens(500), BudgetStatus::Ok);
        // 300 more -> 800/1000 = 80%, Warning (crosses threshold)
        assert_eq!(budget.add_tokens(300), BudgetStatus::Warning);
        // 199 more -> 999/1000, Ok (warning already emitted)
        assert_eq!(budget.add_tokens(199), BudgetStatus::Ok);
        // 1 more -> exactly 1000, Exhausted
        assert_eq!(budget.add_tokens(1), BudgetStatus::Exhausted);
    }

    #[test]
    fn add_tokens_returns_exhausted_when_jumping_over_budget() {
        let budget = RequestBudget::new(1000);
        assert_eq!(budget.add_tokens(1500), BudgetStatus::Exhausted);
    }

    #[test]
    fn add_tokens_warning_then_exhausted() {
        let budget = RequestBudget::new(100);
        // threshold = 80
        assert_eq!(budget.add_tokens(79), BudgetStatus::Ok);
        assert_eq!(budget.add_tokens(1), BudgetStatus::Warning); // crosses 80
        assert_eq!(budget.add_tokens(20), BudgetStatus::Exhausted); // reaches 100
    }

    #[tokio::test]
    async fn parallel_add_tokens_warning_fires_at_most_once() {
        let budget = RequestBudget::new(10_000);

        let mut handles = Vec::new();
        // Spawn 100 tasks, each adding 100 tokens = 10,000 total
        for _ in 0..100 {
            let b = budget.clone();
            handles.push(tokio::spawn(async move { b.add_tokens(100) }));
        }

        let mut warning_count = 0;
        for handle in handles {
            let status = handle.await.unwrap();
            if status == BudgetStatus::Warning {
                warning_count += 1;
            }
        }

        assert!(
            warning_count <= 1,
            "Warning fired {warning_count} times, expected at most 1"
        );
    }

    #[test]
    fn remaining_is_correct() {
        let budget = RequestBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        budget.add_tokens(300);
        assert_eq!(budget.remaining(), 700);
        budget.add_tokens(800);
        // Saturating: 1000 - 1100 = 0
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn percentage_is_correct() {
        let budget = RequestBudget::new(1000);
        assert!((budget.percentage() - 0.0).abs() < f32::EPSILON);
        budget.add_tokens(500);
        assert!((budget.percentage() - 50.0).abs() < f32::EPSILON);
        budget.add_tokens(500);
        assert!((budget.percentage() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zero_budget_percentage() {
        let budget = RequestBudget::new(0);
        assert!((budget.percentage() - 100.0).abs() < f32::EPSILON);
    }
}
