//! Agent execution engine for Boternity.
//!
//! The agent module coordinates the conversation loop between users and bots:
//! - `AgentContext`: holds conversation state, personality content, and token budget
//! - `SystemPromptBuilder`: assembles soul + identity + user + memories into an XML-tagged prompt
//! - `AgentEngine`: sends messages through the LLM provider and returns streaming events

pub mod context;
pub mod engine;
pub mod prompt;
pub mod summarizer;
pub mod title;
