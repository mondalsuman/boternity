//! Slash command parsing and execution for the chat loop.
//!
//! Commands start with `/` and provide in-chat controls for session
//! management, help, and memory injection.

use console::style;

/// Available slash commands in the chat loop.
#[derive(Debug, PartialEq)]
pub enum ChatCommand {
    /// Show available commands.
    Help,
    /// Clear the terminal screen.
    Clear,
    /// Exit the chat session.
    Exit,
    /// Start a new session with the same bot.
    New,
    /// Show conversation history for this session.
    History,
    /// Manually inject a memory.
    Remember(String),
    /// Unknown command.
    Unknown(String),
}

/// Parse user input as a slash command.
///
/// Returns `None` if the input doesn't start with `/`.
pub fn parse(input: &str) -> Option<ChatCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim().to_string());

    match cmd.as_str() {
        "/help" | "/h" | "/?" => Some(ChatCommand::Help),
        "/clear" | "/cls" => Some(ChatCommand::Clear),
        "/exit" | "/quit" | "/q" => Some(ChatCommand::Exit),
        "/new" => Some(ChatCommand::New),
        "/history" => Some(ChatCommand::History),
        "/remember" | "/rem" => {
            if let Some(fact) = arg {
                if fact.is_empty() {
                    Some(ChatCommand::Unknown("/remember requires a fact".to_string()))
                } else {
                    Some(ChatCommand::Remember(fact))
                }
            } else {
                Some(ChatCommand::Unknown("/remember requires a fact".to_string()))
            }
        }
        other => Some(ChatCommand::Unknown(other.to_string())),
    }
}

/// Print the help text listing all available commands.
pub fn print_help() {
    println!();
    println!("  {}", style("Available commands:").bold());
    println!();
    println!(
        "  {}    {}",
        style("/help").cyan(),
        "Show this help message"
    );
    println!(
        "  {}   {}",
        style("/clear").cyan(),
        "Clear the screen"
    );
    println!(
        "  {}    {}",
        style("/exit").cyan(),
        "End the chat session"
    );
    println!(
        "  {}     {}",
        style("/new").cyan(),
        "Start a new session"
    );
    println!(
        "  {} {}",
        style("/history").cyan(),
        "Show conversation history"
    );
    println!(
        "  {}  {}",
        style("/remember").cyan(),
        "Save a fact to memory"
    );
    println!();
    println!(
        "  {}",
        style("Ctrl+D to exit, Ctrl+C safe (no message loss)").dim()
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_help() {
        assert_eq!(parse("/help"), Some(ChatCommand::Help));
        assert_eq!(parse("/h"), Some(ChatCommand::Help));
        assert_eq!(parse("/?"), Some(ChatCommand::Help));
    }

    #[test]
    fn test_parse_exit() {
        assert_eq!(parse("/exit"), Some(ChatCommand::Exit));
        assert_eq!(parse("/quit"), Some(ChatCommand::Exit));
        assert_eq!(parse("/q"), Some(ChatCommand::Exit));
    }

    #[test]
    fn test_parse_clear() {
        assert_eq!(parse("/clear"), Some(ChatCommand::Clear));
        assert_eq!(parse("/cls"), Some(ChatCommand::Clear));
    }

    #[test]
    fn test_parse_remember() {
        assert_eq!(
            parse("/remember User likes Rust"),
            Some(ChatCommand::Remember("User likes Rust".to_string()))
        );
    }

    #[test]
    fn test_parse_not_command() {
        assert_eq!(parse("hello world"), None);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(
            parse("/foo"),
            Some(ChatCommand::Unknown("/foo".to_string()))
        );
    }
}
