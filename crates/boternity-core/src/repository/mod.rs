//! Repository trait definitions (ports).
//!
//! These traits define the storage interface that the infrastructure layer
//! (boternity-infra) implements. The core crate never depends on any
//! specific storage technology.

pub mod bot;
pub mod message;
pub mod secret;
pub mod soul;
pub mod workflow;

/// Sort order for list queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Desc
    }
}
