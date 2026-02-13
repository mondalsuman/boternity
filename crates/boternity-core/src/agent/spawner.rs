//! Spawn instruction parser for Boternity agent responses.
//!
//! Parses LLM response text for `<spawn_agents>` XML blocks, extracting
//! spawn mode (parallel/sequential) and task descriptions. Also provides
//! a helper to extract the text preceding the spawn block (the "pre-spawn
//! message" the bot says before delegating).

use boternity_types::agent::{SpawnInstruction, SpawnMode};

/// Parse spawn instructions from an LLM response.
///
/// Looks for a `<spawn_agents>` XML block in the response text. If found,
/// extracts the spawn mode and task descriptions from `<agent>` elements.
///
/// Returns `None` if:
/// - No `<spawn_agents>` block is found
/// - The block contains no `<agent task="...">` elements
///
/// Only the first `<spawn_agents>` block is parsed (one spawn per response).
///
/// # XML Format
///
/// ```xml
/// <spawn_agents mode="parallel">
///   <agent task="Research the history of quantum computing" />
///   <agent task="Summarize recent breakthroughs" />
/// </spawn_agents>
/// ```
pub fn parse_spawn_instructions(response: &str) -> Option<SpawnInstruction> {
    let start_idx = response.find("<spawn_agents")?;
    let end_tag = "</spawn_agents>";
    let end_idx = response.find(end_tag)?;
    let block = &response[start_idx..end_idx + end_tag.len()];

    let mode = if block.contains(r#"mode="sequential""#) {
        SpawnMode::Sequential
    } else {
        SpawnMode::Parallel
    };

    // Extract task="..." attributes from <agent> elements within the block.
    // Uses a proper attribute parser that handles quotes correctly.
    let mut tasks = Vec::new();
    let task_prefix = r#"task=""#;
    let mut search_from = 0;

    while let Some(pos) = block[search_from..].find(task_prefix) {
        let abs_pos = search_from + pos + task_prefix.len();
        // Read until the next unescaped double quote
        if let Some(end_quote) = find_closing_quote(&block[abs_pos..]) {
            let task_text = &block[abs_pos..abs_pos + end_quote];
            if !task_text.is_empty() {
                tasks.push(task_text.to_string());
            }
            search_from = abs_pos + end_quote + 1;
        } else {
            break;
        }
    }

    if tasks.is_empty() {
        return None;
    }

    Some(SpawnInstruction { mode, tasks })
}

/// Extract text before the `<spawn_agents>` tag, trimmed.
///
/// Returns the "pre-spawn message" -- the text the bot says before delegating
/// to sub-agents (e.g., "I'll break this down into sub-tasks...").
///
/// If no `<spawn_agents>` block is found, returns the full response trimmed.
pub fn extract_text_before_spawn(response: &str) -> &str {
    match response.find("<spawn_agents") {
        Some(idx) => response[..idx].trim(),
        None => response.trim(),
    }
}

/// Find the position of the next unescaped double quote in a string.
///
/// Handles escaped quotes (`\"`) by skipping them.
fn find_closing_quote(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\\' {
            // Skip the next character (escaped)
            let _ = chars.next();
            continue;
        }
        if ch == '"' {
            return Some(idx);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_parallel_mode_3_tasks() {
        let response = r#"I'll break this down.

<spawn_agents mode="parallel">
  <agent task="Research the history of quantum computing" />
  <agent task="Summarize recent breakthroughs in quantum error correction" />
  <agent task="List top quantum computing companies" />
</spawn_agents>"#;

        let instruction = parse_spawn_instructions(response).unwrap();
        assert_eq!(instruction.mode, SpawnMode::Parallel);
        assert_eq!(instruction.tasks.len(), 3);
        assert_eq!(
            instruction.tasks[0],
            "Research the history of quantum computing"
        );
        assert_eq!(
            instruction.tasks[1],
            "Summarize recent breakthroughs in quantum error correction"
        );
        assert_eq!(
            instruction.tasks[2],
            "List top quantum computing companies"
        );
    }

    #[test]
    fn test_parse_sequential_mode_2_tasks() {
        let response = r#"<spawn_agents mode="sequential">
  <agent task="First, gather the raw data" />
  <agent task="Then, analyze the patterns" />
</spawn_agents>"#;

        let instruction = parse_spawn_instructions(response).unwrap();
        assert_eq!(instruction.mode, SpawnMode::Sequential);
        assert_eq!(instruction.tasks.len(), 2);
        assert_eq!(instruction.tasks[0], "First, gather the raw data");
        assert_eq!(instruction.tasks[1], "Then, analyze the patterns");
    }

    #[test]
    fn test_parse_no_spawn_block_returns_none() {
        let response = "Just a regular response with no spawn block.";
        assert!(parse_spawn_instructions(response).is_none());
    }

    #[test]
    fn test_parse_empty_agent_list_returns_none() {
        let response = r#"<spawn_agents mode="parallel">
</spawn_agents>"#;
        assert!(parse_spawn_instructions(response).is_none());
    }

    #[test]
    fn test_parse_default_mode_is_parallel_when_no_mode_attr() {
        let response = r#"<spawn_agents>
  <agent task="Do something" />
</spawn_agents>"#;

        let instruction = parse_spawn_instructions(response).unwrap();
        assert_eq!(instruction.mode, SpawnMode::Parallel);
        assert_eq!(instruction.tasks.len(), 1);
        assert_eq!(instruction.tasks[0], "Do something");
    }

    #[test]
    fn test_extract_text_before_spawn_returns_trimmed_text() {
        let response = r#"I'll break this into sub-tasks for you.

<spawn_agents mode="parallel">
  <agent task="Task 1" />
</spawn_agents>"#;

        let text = extract_text_before_spawn(response);
        assert_eq!(text, "I'll break this into sub-tasks for you.");
    }

    #[test]
    fn test_extract_text_before_spawn_returns_full_text_when_no_block() {
        let response = "Just a regular response with no spawn block.";
        let text = extract_text_before_spawn(response);
        assert_eq!(text, "Just a regular response with no spawn block.");
    }

    #[test]
    fn test_task_text_with_special_characters_preserved() {
        let response = r#"<spawn_agents mode="parallel">
  <agent task="Research: the history of AI (1950s-present), including key breakthroughs" />
  <agent task="Compare architectures -- transformers vs. RNNs, noting pros/cons" />
</spawn_agents>"#;

        let instruction = parse_spawn_instructions(response).unwrap();
        assert_eq!(instruction.tasks.len(), 2);
        assert_eq!(
            instruction.tasks[0],
            "Research: the history of AI (1950s-present), including key breakthroughs"
        );
        assert_eq!(
            instruction.tasks[1],
            "Compare architectures -- transformers vs. RNNs, noting pros/cons"
        );
    }

    #[test]
    fn test_multiple_spawn_blocks_only_first_parsed() {
        let response = r#"First block:
<spawn_agents mode="parallel">
  <agent task="Task from first block" />
</spawn_agents>

Second block:
<spawn_agents mode="sequential">
  <agent task="Task from second block" />
</spawn_agents>"#;

        let instruction = parse_spawn_instructions(response).unwrap();
        assert_eq!(instruction.mode, SpawnMode::Parallel);
        assert_eq!(instruction.tasks.len(), 1);
        assert_eq!(instruction.tasks[0], "Task from first block");
    }

    #[test]
    fn test_parse_incomplete_spawn_block_returns_none() {
        // Has start tag but no end tag
        let response = r#"<spawn_agents mode="parallel">
  <agent task="Orphaned task" />"#;
        assert!(parse_spawn_instructions(response).is_none());
    }

    #[test]
    fn test_extract_text_before_spawn_empty_prefix() {
        let response = r#"<spawn_agents mode="parallel">
  <agent task="Immediate spawn" />
</spawn_agents>"#;

        let text = extract_text_before_spawn(response);
        assert_eq!(text, "");
    }
}
