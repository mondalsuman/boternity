//! Request context for agent hierarchy execution.
//!
//! `RequestContext` bundles the shared state that flows through an agent tree:
//! token budget, workspace, cancellation token, and cycle detector. The `child()`
//! method creates a derived context for sub-agent spawning with shared budget
//! and workspace but an independent (child) cancellation token.

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::budget::RequestBudget;
use super::cycle_detector::CycleDetector;
use super::workspace::SharedWorkspace;

/// Shared execution context propagated through an agent hierarchy.
///
/// All `Arc`-backed fields (budget, workspace, cycle_detector) are shared
/// across parent and child contexts. The cancellation token forms a tree:
/// cancelling a parent cancels all children, but not vice versa.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique identifier for this request (shared across the tree).
    pub request_id: Uuid,
    /// Token budget shared across all agents in the hierarchy.
    pub budget: RequestBudget,
    /// Key-value workspace shared across all agents.
    pub workspace: SharedWorkspace,
    /// Cancellation token -- child tokens are derived from the parent.
    pub cancellation: CancellationToken,
    /// Cycle detector shared across the hierarchy.
    pub cycle_detector: CycleDetector,
    /// Depth in the agent tree (root = 0).
    pub depth: u8,
}

impl RequestContext {
    /// Create a root context (depth 0) with the given request ID and budget.
    pub fn new(request_id: Uuid, budget: RequestBudget) -> Self {
        Self {
            request_id,
            budget,
            workspace: SharedWorkspace::new(),
            cancellation: CancellationToken::new(),
            cycle_detector: CycleDetector::new(),
            depth: 0,
        }
    }

    /// Create a child context for sub-agent spawning.
    ///
    /// The child shares the same budget, workspace, and cycle detector
    /// (all `Arc`-backed). It receives a child cancellation token
    /// (cancelled when the parent is cancelled) and depth + 1.
    pub fn child(&self) -> Self {
        Self {
            request_id: self.request_id,
            budget: self.budget.clone(),
            workspace: self.workspace.clone(),
            cancellation: self.cancellation.child_token(),
            cycle_detector: self.cycle_detector.clone(),
            depth: self.depth.saturating_add(1),
        }
    }

    /// Check whether this context has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Cancel this context (and all child contexts derived from it).
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_uuid() -> Uuid {
        Uuid::now_v7()
    }

    #[test]
    fn new_creates_root_at_depth_0() {
        let ctx = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        assert_eq!(ctx.depth, 0);
        assert!(!ctx.is_cancelled());
    }

    #[test]
    fn child_increments_depth() {
        let root = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        let child = root.child();
        assert_eq!(child.depth, 1);
        let grandchild = child.child();
        assert_eq!(grandchild.depth, 2);
    }

    #[test]
    fn child_shares_same_budget() {
        let root = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        let child = root.child();
        child.budget.add_tokens(100);
        assert_eq!(root.budget.tokens_used(), 100);
    }

    #[test]
    fn child_shares_same_workspace() {
        let root = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        let child = root.child();
        child.workspace.set("key".to_string(), json!("value"));
        assert_eq!(root.workspace.get("key"), Some(json!("value")));
    }

    #[test]
    fn cancelling_parent_cancels_child() {
        let root = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        let child = root.child();
        let grandchild = child.child();

        assert!(!child.is_cancelled());
        assert!(!grandchild.is_cancelled());

        root.cancel();

        assert!(child.is_cancelled());
        assert!(grandchild.is_cancelled());
    }

    #[test]
    fn cancelling_child_does_not_cancel_parent() {
        let root = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        let child = root.child();

        child.cancel();

        assert!(!root.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn child_shares_request_id() {
        let id = test_uuid();
        let root = RequestContext::new(id, RequestBudget::new(1000));
        let child = root.child();
        assert_eq!(child.request_id, id);
    }

    #[test]
    fn child_shares_cycle_detector() {
        use crate::agent::cycle_detector::CycleCheckResult;

        let root = RequestContext::new(test_uuid(), RequestBudget::new(1000));
        let child = root.child();

        // Register in root, count increments in child's detector too
        root.cycle_detector.check_and_register("task", 0);
        child.cycle_detector.check_and_register("task", 1);
        child.cycle_detector.check_and_register("task", 2);

        // Fourth occurrence exceeds default threshold of 3
        let result = root.cycle_detector.check_and_register("task", 0);
        assert!(matches!(result, CycleCheckResult::CycleDetected { .. }));
    }
}
