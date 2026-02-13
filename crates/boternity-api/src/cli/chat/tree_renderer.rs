//! Tree-indented rendering for sub-agent output.
//!
//! Renders sub-agent headers, text continuation lines, completion stats,
//! and safety warnings using Unicode box-drawing characters and colored
//! labels. Each sub-agent is shown as a branch in a tree rooted at the
//! parent agent.

use console::style;

// Unicode box-drawing characters for tree rendering.
const BRANCH: &str = "\u{251C}\u{2500}\u{2500}"; // +--
const LAST: &str = "\u{2514}\u{2500}\u{2500}"; // L--
const PIPE: &str = "\u{2502}   "; // |
const SPACE: &str = "    ";

/// Render a header line when a sub-agent is spawned.
///
/// Example output: `  +-- agent-1: Research quantum computing history...`
///
/// - `depth`: nesting level (0 = direct child of root)
/// - `index`: 0-based position among siblings
/// - `total`: total number of siblings in this batch
/// - `task`: task description (truncated to 60 chars with ellipsis)
pub fn render_agent_header(depth: u8, index: usize, total: usize, task: &str) -> String {
    let indent = "  ".repeat(depth as usize);
    let branch = if index == total.saturating_sub(1) {
        LAST
    } else {
        BRANCH
    };
    let label = format!("agent-{}", index + 1);
    let truncated = truncate_task(task, 60);

    format!(
        "{indent}{branch} {}: {truncated}",
        style(label).cyan()
    )
}

/// Render a text continuation line under a sub-agent.
///
/// Uses PIPE or SPACE for indent continuation depending on whether
/// this agent is the last sibling.
pub fn render_agent_text_line(depth: u8, index: usize, total: usize, text: &str) -> String {
    let indent = "  ".repeat(depth as usize);
    let continuation = if index == total.saturating_sub(1) {
        SPACE
    } else {
        PIPE
    };

    format!("{indent}{continuation}{text}")
}

/// Render a completion stats line for a sub-agent.
///
/// Example: `  |   | 2,450 tokens . 3.2s`
pub fn render_agent_completion(
    depth: u8,
    index: usize,
    total: usize,
    tokens: u32,
    duration_ms: u64,
) -> String {
    let indent = "  ".repeat(depth as usize);
    let continuation = if index == total.saturating_sub(1) {
        SPACE
    } else {
        PIPE
    };
    let seconds = duration_ms as f64 / 1000.0;

    format!(
        "{indent}{continuation}{} {} tokens {} {:.1}s",
        style("|").dim(),
        style(format_tokens_human(tokens)).dim(),
        style("\u{00b7}").dim(),
        style(seconds).dim(),
    )
}

/// Render a depth limit warning.
///
/// Example: `  ! Depth limit reached (attempted depth 4, max 3)`
pub fn render_depth_limit_warning(depth: u8, max: u8) -> String {
    format!(
        "  {} {}",
        style("!").yellow().bold(),
        style(format!(
            "Depth limit reached (attempted depth {depth}, max {max})"
        ))
        .yellow()
    )
}

/// Render a cycle detection warning.
///
/// Example: `  ! Cycle detected: A -> B -> A`
pub fn render_cycle_warning(description: &str) -> String {
    format!(
        "  {} {}",
        style("!").yellow().bold(),
        style(format!("Cycle detected: {description}")).yellow()
    )
}

/// Format a token count with comma separators for human readability.
///
/// Example: `1234567` -> `"1,234,567"`
pub fn format_tokens_human(n: u32) -> String {
    let s = n.to_string();
    let len = s.len();
    if len <= 3 {
        return s;
    }

    let mut result = String::with_capacity(len + (len - 1) / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Truncate a task description to the given max length, appending ellipsis.
fn truncate_task(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_agent_header_depth_0() {
        let header = render_agent_header(0, 0, 3, "Research quantum computing");
        // Should contain the branch character and cyan agent label
        assert!(header.contains("agent-1"));
        assert!(header.contains("Research quantum computing"));
        // Should use BRANCH (not LAST) since index=0, total=3
        assert!(header.contains(BRANCH));
    }

    #[test]
    fn render_agent_header_depth_1() {
        let header = render_agent_header(1, 0, 2, "Sub-sub task");
        // Depth 1 = 2 spaces of indent before the branch
        assert!(header.starts_with("  "));
        assert!(header.contains("agent-1"));
    }

    #[test]
    fn render_agent_header_depth_2() {
        let header = render_agent_header(2, 1, 2, "Deep task");
        // Depth 2 = 4 spaces of indent
        assert!(header.starts_with("    "));
        assert!(header.contains("agent-2"));
    }

    #[test]
    fn render_agent_header_last_item_uses_last_branch() {
        let header = render_agent_header(0, 2, 3, "Last task");
        // index=2, total=3 -> last item, should use LAST
        assert!(header.contains(LAST));
        assert!(!header.contains(BRANCH));
    }

    #[test]
    fn render_agent_header_truncates_long_task() {
        let long_task = "a".repeat(100);
        let header = render_agent_header(0, 0, 1, &long_task);
        // Should be truncated to 60 chars + "..."
        assert!(header.contains(&format!("{}...", "a".repeat(60))));
    }

    #[test]
    fn format_tokens_human_zero() {
        assert_eq!(format_tokens_human(0), "0");
    }

    #[test]
    fn format_tokens_human_small() {
        assert_eq!(format_tokens_human(42), "42");
        assert_eq!(format_tokens_human(999), "999");
    }

    #[test]
    fn format_tokens_human_thousands() {
        assert_eq!(format_tokens_human(1_000), "1,000");
        assert_eq!(format_tokens_human(12_345), "12,345");
    }

    #[test]
    fn format_tokens_human_millions() {
        assert_eq!(format_tokens_human(1_234_567), "1,234,567");
    }

    #[test]
    fn render_agent_completion_contains_tokens_and_duration() {
        let line = render_agent_completion(0, 0, 2, 2450, 3200);
        assert!(line.contains("2,450"));
        assert!(line.contains("3.2s"));
    }

    #[test]
    fn render_depth_limit_warning_contains_values() {
        let warning = render_depth_limit_warning(4, 3);
        assert!(warning.contains("depth 4"));
        assert!(warning.contains("max 3"));
    }

    #[test]
    fn render_cycle_warning_contains_description() {
        let warning = render_cycle_warning("A -> B -> A");
        assert!(warning.contains("Cycle detected: A -> B -> A"));
    }

    #[test]
    fn render_agent_text_line_not_last() {
        let line = render_agent_text_line(0, 0, 2, "some text");
        // Not last -> PIPE continuation
        assert!(line.contains(PIPE));
        assert!(line.contains("some text"));
    }

    #[test]
    fn render_agent_text_line_last() {
        let line = render_agent_text_line(0, 1, 2, "some text");
        // Last -> SPACE continuation
        assert!(line.contains(SPACE));
        assert!(line.contains("some text"));
    }
}
