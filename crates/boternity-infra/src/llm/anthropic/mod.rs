//! Anthropic Claude LLM provider implementation.
//!
//! This module provides the [`AnthropicProvider`] which implements the
//! [`LlmProvider`](boternity_core::llm::provider::LlmProvider) trait for
//! the Anthropic Messages API, including full SSE streaming support.

pub mod client;
pub mod streaming;
pub mod types;

pub use client::AnthropicProvider;
