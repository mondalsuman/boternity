//! LLM provider abstractions for Boternity.
//!
//! This module defines the core traits and utilities for LLM provider integration:
//! - `LlmProvider`: RPITIT trait for concrete provider implementations
//! - `BoxLlmProvider`: Object-safe wrapper for dynamic dispatch
//! - `TokenBudget`: Context window allocation management

pub mod box_provider;
pub mod provider;
pub mod token_budget;
pub mod types;
