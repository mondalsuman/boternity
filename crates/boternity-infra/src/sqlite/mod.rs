//! SQLite storage layer.
//!
//! Repository implementations backed by SQLite with WAL mode and split
//! read/write connection pools.

pub mod bot;
pub mod pool;
pub mod secret;
pub mod soul;
