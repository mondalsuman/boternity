//! Interactive CLI chat experience for Boternity.
//!
//! This module implements the full chat loop: streaming LLM responses with
//! markdown rendering, thinking spinners, welcome banners, slash commands,
//! and session persistence. Entry point: `loop_runner::run_chat_loop`.

pub mod banner;
pub mod commands;
pub mod input;
pub mod loop_runner;
pub mod renderer;
