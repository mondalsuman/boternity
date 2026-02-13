//! Event bus for agent hierarchy communication.
//!
//! Provides an `EventBus` that distributes `AgentEvent` messages to all
//! subscribers via a `tokio::sync::broadcast` channel.

pub mod bus;

pub use bus::EventBus;
