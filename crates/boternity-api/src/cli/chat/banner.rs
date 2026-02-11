//! Welcome banner display for chat sessions.
//!
//! Prints a styled banner when a chat session starts, showing the bot's
//! identity, model, and session information.

use console::style;

/// Print the welcome banner at the start of a chat session.
///
/// Displays the bot's emoji, name, description, model, and session ID
/// with styled formatting. Includes a hint about slash commands.
pub fn print_welcome_banner(
    name: &str,
    emoji: Option<&str>,
    description: &str,
    model: &str,
    session_id: &str,
) {
    let emoji_str = emoji.unwrap_or("*");

    println!();
    println!(
        "  {} {}",
        emoji_str,
        style(name).cyan().bold()
    );
    println!(
        "  {}",
        style(description).dim()
    );
    println!();
    println!(
        "  {}  {}",
        style("Model:").bold(),
        style(model).dim()
    );
    println!(
        "  {}  {}",
        style("Session:").bold(),
        style(&session_id[..8.min(session_id.len())]).dim()
    );
    println!();
    println!(
        "  {}",
        style("Type /help for commands, Ctrl+D to exit").dim()
    );
    println!(
        "  {}",
        style("---").dim()
    );
    println!();
}
