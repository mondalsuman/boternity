//! AWS Bedrock LLM provider implementation.
//!
//! Implements [`LlmProvider`] for the AWS Bedrock Runtime API, using
//! Bearer token authentication and the Bedrock event stream binary protocol.

mod client;
mod streaming;
pub mod types;

pub use client::BedrockProvider;
