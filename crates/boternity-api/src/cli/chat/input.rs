//! Async readline input handling for the chat loop.
//!
//! Wraps `rustyline_async::Readline` to provide async line reading with
//! proper handling of EOF (Ctrl+D) and interrupt (Ctrl+C) signals.

use rustyline_async::{Readline, ReadlineError, SharedWriter};

/// Events produced by the input handler.
#[derive(Debug)]
pub enum InputEvent {
    /// User submitted a message.
    Message(String),
    /// End of file (Ctrl+D).
    Eof,
    /// Interrupt signal (Ctrl+C).
    Interrupted,
}

/// Async input handler wrapping rustyline_async.
pub struct ChatInput {
    rl: Readline,
}

impl ChatInput {
    /// Create a new chat input handler with the given initial prompt.
    ///
    /// Returns the input handler and a `SharedWriter` that can be used to
    /// print output without interfering with the readline prompt.
    pub fn new(prompt: String) -> Result<(Self, SharedWriter), ReadlineError> {
        let (rl, stdout) = Readline::new(prompt)?;
        Ok((Self { rl }, stdout))
    }

    /// Update the prompt displayed to the user.
    pub fn update_prompt(&mut self, prompt: &str) {
        let _ = self.rl.update_prompt(prompt);
    }

    /// Read a line of input.
    ///
    /// Returns an `InputEvent` indicating what the user did:
    /// - `Message(text)` for a submitted line
    /// - `Eof` for Ctrl+D
    /// - `Interrupted` for Ctrl+C
    pub async fn read_line(&mut self) -> InputEvent {
        match self.rl.readline().await {
            Ok(rustyline_async::ReadlineEvent::Line(line)) => {
                let trimmed = line.trim().to_string();
                InputEvent::Message(trimmed)
            }
            Ok(rustyline_async::ReadlineEvent::Eof) => InputEvent::Eof,
            Ok(rustyline_async::ReadlineEvent::Interrupted) => InputEvent::Interrupted,
            Err(_) => InputEvent::Eof,
        }
    }

    /// Clear the terminal screen.
    pub fn clear(&mut self) {
        let _ = self.rl.clear();
    }
}
