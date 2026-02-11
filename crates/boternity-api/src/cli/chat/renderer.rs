//! Terminal markdown rendering with syntax-highlighted code blocks.
//!
//! `ChatRenderer` combines `termimad` for prose and `syntect` for code block
//! syntax highlighting. During streaming, tokens are printed raw; once the
//! full response is collected, it is rendered as formatted markdown.

use std::io::Write;

use crossterm::style::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use termimad::MadSkin;

/// Terminal markdown renderer with syntax highlighting.
pub struct ChatRenderer {
    skin: MadSkin,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl ChatRenderer {
    /// Create a new renderer with an optional accent color for the bot.
    pub fn new(accent_color: Option<Color>) -> Self {
        let mut skin = MadSkin::default_dark();

        // Apply accent color to headers and bold text if provided
        if let Some(color) = accent_color {
            let tc = Self::crossterm_to_termimad(color);
            skin.bold.set_fg(tc);
            skin.headers[0].set_fg(tc);
            skin.headers[1].set_fg(tc);
        }

        // Style inline code
        skin.inline_code
            .set_fg(termimad::crossterm::style::Color::Yellow);

        Self {
            skin,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Render a complete markdown response with syntax-highlighted code blocks.
    ///
    /// Code fences with a language tag are highlighted via syntect; everything
    /// else is rendered through termimad.
    pub fn render_final(&self, markdown: &str) -> String {
        let mut output = String::new();
        let mut in_code_block = false;
        let mut code_lang = String::new();
        let mut code_buf = String::new();

        for line in markdown.lines() {
            if line.starts_with("```") && !in_code_block {
                // Opening code fence
                in_code_block = true;
                code_lang = line.trim_start_matches('`').trim().to_string();
                code_buf.clear();
            } else if line.starts_with("```") && in_code_block {
                // Closing code fence -- render the accumulated code
                in_code_block = false;
                let highlighted = self.highlight_code(&code_buf, &code_lang);
                output.push_str(&highlighted);
                output.push('\n');
            } else if in_code_block {
                code_buf.push_str(line);
                code_buf.push('\n');
            } else {
                // Prose line -- render through termimad
                let rendered = self.skin.term_text(line);
                output.push_str(&format!("{rendered}"));
            }
        }

        // Handle unclosed code block
        if in_code_block && !code_buf.is_empty() {
            let highlighted = self.highlight_code(&code_buf, &code_lang);
            output.push_str(&highlighted);
        }

        output
    }

    /// Print a single streaming token (raw, no formatting).
    pub fn print_streaming_token(&self, token: &str) {
        print!("{token}");
        let _ = std::io::stdout().flush();
    }

    /// Print the stats footer after a bot response.
    ///
    /// Format: "| {tokens} tokens . {time}s . {model}"
    pub fn print_stats_footer(&self, tokens: u32, response_ms: u64, model: &str) {
        let seconds = response_ms as f64 / 1000.0;
        let footer = format!(
            "\n  {} {} tokens {} {:.1}s {} {}",
            console::style("|").dim(),
            console::style(tokens).dim(),
            console::style("\u{00b7}").dim(),
            console::style(seconds).dim(),
            console::style("\u{00b7}").dim(),
            console::style(model).dim(),
        );
        println!("{footer}");
    }

    /// Highlight a code block using syntect.
    fn highlight_code(&self, code: &str, lang: &str) -> String {
        let syntax = if lang.is_empty() {
            self.syntax_set.find_syntax_plain_text()
        } else {
            self.syntax_set
                .find_syntax_by_token(lang)
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
        };

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut h = HighlightLines::new(syntax, theme);

        let mut output = String::new();
        output.push_str(&format!("  {}\n", console::style(format!("--- {lang} ---")).dim()));

        for line in code.lines() {
            let ranges: Vec<(Style, &str)> = h
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            output.push_str(&format!("  {escaped}\x1b[0m\n"));
        }

        output
    }

    /// Convert a crossterm Color to termimad Color.
    fn crossterm_to_termimad(color: Color) -> termimad::crossterm::style::Color {
        match color {
            Color::Cyan => termimad::crossterm::style::Color::Cyan,
            Color::Green => termimad::crossterm::style::Color::Green,
            Color::Yellow => termimad::crossterm::style::Color::Yellow,
            Color::Magenta => termimad::crossterm::style::Color::Magenta,
            Color::Blue => termimad::crossterm::style::Color::Blue,
            Color::Red => termimad::crossterm::style::Color::Red,
            Color::Rgb { r, g, b } => termimad::crossterm::style::Color::Rgb { r, g, b },
            _ => termimad::crossterm::style::Color::Cyan,
        }
    }
}
