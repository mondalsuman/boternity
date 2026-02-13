//! Budget display formatting for CLI sub-agent execution.
//!
//! Renders live budget counters, warning prompts, exhaustion messages,
//! and completion stats with cost estimates. Colors change at the 80%
//! threshold to provide visual feedback on budget consumption.

use console::style;

use super::tree_renderer::format_tokens_human;

/// Render a budget counter line showing tokens used vs total.
///
/// Example: `  [tokens: 12,450 / 500,000]`
///
/// The counter is yellow when >= 80% consumed, dim otherwise.
pub fn render_budget_counter(used: u32, total: u32) -> String {
    let percentage = if total == 0 {
        100.0
    } else {
        used as f64 / total as f64 * 100.0
    };
    let text = format!(
        "  [tokens: {} / {}]",
        format_tokens_human(used),
        format_tokens_human(total),
    );

    if percentage >= 80.0 {
        format!("{}", style(text).yellow())
    } else {
        format!("{}", style(text).dim())
    }
}

/// Render the budget warning prompt (shown when 80% is reached).
///
/// Example: `  ! Budget 80% used. Continue? (y/n)`
pub fn render_budget_warning_prompt() -> String {
    format!(
        "  {} {}",
        style("!").yellow().bold(),
        style("Budget 80% used. Continue? (y/n)").yellow().bold()
    )
}

/// Render a budget exhaustion message.
///
/// Example: `  ! Budget exhausted: 500,000 / 500,000 tokens. 2 agents completed, 1 incomplete.`
pub fn render_budget_exhausted(
    used: u32,
    total: u32,
    completed: usize,
    incomplete: usize,
) -> String {
    format!(
        "  {} {} {} agent{} completed, {} incomplete.",
        style("!").red().bold(),
        style(format!(
            "Budget exhausted: {} / {} tokens.",
            format_tokens_human(used),
            format_tokens_human(total),
        ))
        .red(),
        completed,
        if completed == 1 { "" } else { "s" },
        incomplete,
    )
}

/// Render completion stats with cost estimate and duration.
///
/// Example: `  [tokens: 6,740 / 500,000 . ~$0.04 estimated . 8.1s]`
pub fn render_completion_stats(
    tokens_used: u32,
    budget_total: u32,
    cost_estimate: f64,
    duration_secs: f64,
) -> String {
    format!(
        "  {}",
        style(format!(
            "[tokens: {} / {} \u{00b7} ~${:.2} estimated \u{00b7} {:.1}s]",
            format_tokens_human(tokens_used),
            format_tokens_human(budget_total),
            cost_estimate,
            duration_secs,
        ))
        .dim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_budget_counter_low_usage() {
        let counter = render_budget_counter(100_000, 500_000);
        // 20% -> dim styling, contains token counts
        assert!(counter.contains("100,000"));
        assert!(counter.contains("500,000"));
    }

    #[test]
    fn render_budget_counter_high_usage() {
        let counter = render_budget_counter(400_000, 500_000);
        // 80% -> yellow styling
        assert!(counter.contains("400,000"));
        assert!(counter.contains("500,000"));
    }

    #[test]
    fn render_budget_counter_at_80_percent() {
        // Exactly at 80% should trigger yellow
        let counter = render_budget_counter(80, 100);
        assert!(counter.contains("80"));
        assert!(counter.contains("100"));
    }

    #[test]
    fn render_budget_warning_prompt_contains_question() {
        let prompt = render_budget_warning_prompt();
        assert!(prompt.contains("Budget 80% used"));
        assert!(prompt.contains("Continue?"));
        assert!(prompt.contains("y/n"));
    }

    #[test]
    fn render_budget_exhausted_contains_counts() {
        let msg = render_budget_exhausted(500_000, 500_000, 2, 1);
        assert!(msg.contains("500,000"));
        assert!(msg.contains("2 agents completed"));
        assert!(msg.contains("1 incomplete"));
    }

    #[test]
    fn render_budget_exhausted_single_agent() {
        let msg = render_budget_exhausted(100_000, 100_000, 1, 0);
        assert!(msg.contains("1 agent completed")); // singular
        assert!(msg.contains("0 incomplete"));
    }

    #[test]
    fn render_completion_stats_contains_cost() {
        let stats = render_completion_stats(6_740, 500_000, 0.04, 8.1);
        assert!(stats.contains("6,740"));
        assert!(stats.contains("500,000"));
        assert!(stats.contains("$0.04"));
        assert!(stats.contains("estimated"));
        assert!(stats.contains("8.1s"));
    }

    #[test]
    fn render_completion_stats_zero_cost() {
        let stats = render_completion_stats(0, 500_000, 0.0, 0.0);
        assert!(stats.contains("$0.00"));
        assert!(stats.contains("0.0s"));
    }
}
