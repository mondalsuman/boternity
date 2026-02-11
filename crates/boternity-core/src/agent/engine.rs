//! Agent execution engine for Boternity.
//!
//! AgentEngine coordinates the LLM call loop: assembles the request from
//! AgentContext, sends it through BoxLlmProvider, and returns streaming events.
//! Full implementation in Task 2.
