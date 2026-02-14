//! Standalone interactive skill creation wizard.
//!
//! Launched via `bnity skill generate`. Uses SkillBuilder to drive LLM-powered
//! skill generation: description prompt, type selection, capability suggestions,
//! validation, and write to disk.

use anyhow::{Context, Result};
use console::style;
use dialoguer::{Confirm, Input, Select};

use boternity_core::builder::skill_builder::{SkillBuildRequest, SkillBuildType, SkillBuilder};

use crate::state::AppState;

/// Run the standalone interactive skill creation wizard.
///
/// Flow:
/// 1. Prompt for skill description
/// 2. Select skill type (prompt vs WASM)
/// 3. Generate via SkillBuilder::generate_skill
/// 4. Show suggested capabilities with reasons
/// 5. Confirm capabilities
/// 6. Validate with SkillBuilder::validate_skill
/// 7. Write to disk if valid
pub async fn run_skill_create(state: &AppState) -> Result<()> {
    println!();
    println!(
        "  {} Interactive Skill Builder",
        style("*").cyan().bold()
    );
    println!();

    // Step 1: Describe the skill
    let description: String = Input::new()
        .with_prompt("Describe the skill you want to create")
        .interact_text()?;

    // Step 2: Choose skill name
    let suggested_name = slugify_description(&description);
    let name: String = Input::new()
        .with_prompt("Skill name (slug format)")
        .default(suggested_name)
        .interact_text()?;

    // Step 3: Choose skill type
    let type_items = vec![
        "Local (prompt-based, runs natively)",
        "WASM (sandboxed, compiled Rust)",
    ];
    let type_selection = Select::new()
        .with_prompt("Skill type")
        .items(&type_items)
        .default(0)
        .interact()?;

    let skill_type = match type_selection {
        0 => SkillBuildType::Local,
        _ => SkillBuildType::Wasm {
            language: "rust".to_string(),
        },
    };

    // Step 4: Show heuristic capability suggestions before LLM call
    let heuristic_caps = SkillBuilder::suggest_capabilities(&description);
    if !heuristic_caps.is_empty() {
        println!();
        println!(
            "  {} Suggested permissions based on description:",
            style("*").yellow()
        );
        println!();
        for cap in &heuristic_caps {
            println!(
                "    [x] {} -- {}",
                style(&cap.capability).cyan(),
                style(&cap.reason).dim()
            );
        }
        println!();
    }

    // Step 5: Generate via LLM
    println!(
        "  {} Generating skill with LLM...",
        style("*").cyan()
    );

    let provider = state
        .create_single_provider("claude-sonnet-4-20250514")
        .await
        .context("Failed to create LLM provider for skill builder")?;

    let request = SkillBuildRequest {
        name: name.clone(),
        description: description.clone(),
        skill_type,
        capabilities: None, // Let LLM suggest
    };

    let result = SkillBuilder::generate_skill(&provider, &request)
        .await
        .context("Skill generation failed")?;

    println!(
        "  {} Skill generated successfully!",
        style("*").green()
    );
    println!();

    // Step 6: Show generated capabilities with reasons
    if !result.suggested_capabilities.is_empty() {
        println!(
            "  {} Generated capabilities:",
            style("*").yellow()
        );
        println!();
        for cap in &result.suggested_capabilities {
            println!(
                "    [x] {} -- {}",
                style(&cap.capability).cyan(),
                style(&cap.reason).dim()
            );
        }
        println!();

        let accept_all = Confirm::new()
            .with_prompt("Accept all suggested capabilities?")
            .default(true)
            .interact()?;

        if !accept_all {
            println!(
                "  {} You can edit capabilities in the SKILL.md file after creation.",
                style("i").blue()
            );
        }
    }

    // Step 7: Show generated SKILL.md preview
    println!();
    println!(
        "  {} Generated SKILL.md:",
        style("*").cyan()
    );
    println!("{}", style("---").dim());

    // Show first ~30 lines of the generated SKILL.md
    let preview_lines: Vec<&str> = result.skill_md_content.lines().take(30).collect();
    for line in &preview_lines {
        println!("  {line}");
    }
    if result.skill_md_content.lines().count() > 30 {
        println!("  {} (truncated)", style("...").dim());
    }
    println!("{}", style("---").dim());

    // Step 8: Validate
    println!();
    let warnings = SkillBuilder::validate_skill(&provider, &result.skill_md_content)
        .await
        .context("Skill validation failed")?;

    if warnings.is_empty() {
        println!(
            "  {} Validation passed",
            style("*").green()
        );
    } else {
        println!(
            "  {} Validation warnings:",
            style("!").yellow()
        );
        for warning in &warnings {
            println!("    - {warning}");
        }
    }

    // Step 9: Confirm and write
    println!();
    let confirmed = Confirm::new()
        .with_prompt("Create this skill?")
        .default(true)
        .interact()?;

    if !confirmed {
        println!("  Cancelled.");
        return Ok(());
    }

    // Write to disk
    let install_path = state.skill_store.install_skill(
        &name,
        &result.skill_md_content,
        None,
        None,
    )?;

    // Write source code if WASM
    if let Some(ref source_code) = result.source_code {
        let src_dir = install_path.join("src");
        std::fs::create_dir_all(&src_dir)
            .context("Failed to create src directory")?;
        std::fs::write(src_dir.join("lib.rs"), source_code)
            .context("Failed to write source code")?;
    }

    println!();
    println!(
        "  {} Created skill '{}'",
        style("*").green().bold(),
        style(&name).cyan()
    );
    println!("  Path: {}", install_path.display());
    println!();
    println!(
        "  Edit {} to customize.",
        style(format!("{}/SKILL.md", install_path.display())).yellow()
    );
    println!(
        "  Attach to a bot: {}",
        style(format!("bnity skill attach {name} --bot <slug>")).yellow()
    );
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a slug from a description (first few meaningful words, lowercased, hyphenated).
fn slugify_description(description: &str) -> String {
    // Filter to alphanumeric words only, then take first 3
    let words: Vec<String> = description
        .split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .chars()
                .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .take(3)
        .collect();

    words.join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_description_basic() {
        assert_eq!(slugify_description("Fetch web data from APIs"), "fetch-web-data");
    }

    #[test]
    fn test_slugify_description_special_chars() {
        assert_eq!(slugify_description("Read & parse JSON files"), "read-parse-json");
    }

    #[test]
    fn test_slugify_description_short() {
        assert_eq!(slugify_description("Search"), "search");
    }

    #[test]
    fn test_slugify_description_empty() {
        assert_eq!(slugify_description(""), "");
    }
}
