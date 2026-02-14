//! Workflow infrastructure: webhook handlers, filesystem watchers, and execution context.
//!
//! This module provides concrete implementations for workflow trigger
//! subsystems and step execution:
//! - `webhook_handler` -- HMAC-SHA256/bearer token auth, webhook registry
//! - `file_trigger` -- Debounced filesystem watcher with glob filtering
//! - `execution_context` -- Live step execution wiring (Agent/Skill/HTTP)

pub mod execution_context;
pub mod file_trigger;
pub mod webhook_handler;
