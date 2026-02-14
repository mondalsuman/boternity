//! Bot-to-bot message bus with direct messaging, pub/sub channels, and loop prevention.
//!
//! This module provides the runtime messaging infrastructure for inter-bot communication:
//! - `bus` -- `MessageBus` with per-bot mailboxes, pub/sub channels, and send-and-wait
//! - `envelope` -- Helper constructors for `BotMessage`
//! - `router` -- `LoopGuard` with depth, rate, and time-window protection
//! - `handler` -- `MessageProcessor` trait for pluggable message handling pipelines

pub mod bus;
pub mod envelope;
pub mod handler;
pub mod router;

pub use bus::{MessageBus, MessageError};
pub use handler::MessageProcessor;
pub use router::LoopGuard;
