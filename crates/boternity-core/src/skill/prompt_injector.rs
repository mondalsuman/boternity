//! Prompt injection for prompt-based skills.
//!
//! Implements progressive disclosure: Level 1 shows only skill metadata
//! (name + description) so the LLM knows what is available. Level 2
//! injects the full skill body for active skills. Both levels use XML
//! tags to keep skill content clearly delimited in the system prompt.

use std::path::PathBuf;

use boternity_types::skill::SkillManifest;

// ---------------------------------------------------------------------------
// Level 1: Metadata-only XML (progressive disclosure)
// ---------------------------------------------------------------------------

/// Generate XML metadata for all available skills (Level 1 disclosure).
///
/// Returns an `<available_skills>` block containing each skill's name and
/// description. This lets the LLM know what skills exist without loading
/// their full bodies.
///
/// ```xml
/// <available_skills>
///   <skill name="web-search">Search the web for information</skill>
///   <skill name="code-review">Review code for quality issues</skill>
/// </available_skills>
/// ```
pub fn generate_skill_metadata_xml(skills: &[(SkillManifest, PathBuf)]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut xml = String::from("<available_skills>\n");

    for (manifest, _path) in skills {
        xml.push_str(&format!(
            "  <skill name=\"{}\">{}</skill>\n",
            manifest.name, manifest.description
        ));
    }

    xml.push_str("</available_skills>");
    xml
}

// ---------------------------------------------------------------------------
// Level 2: Full skill body injection
// ---------------------------------------------------------------------------

/// Inject active skill prompts into the base system prompt (Level 2 disclosure).
///
/// Each active skill's full body is wrapped in `<skill name="...">` tags and
/// inserted after the `</identity>` section. If no `</identity>` tag is found,
/// the skills are appended at the end.
///
/// The `active_skills` parameter contains `(manifest, body)` pairs where `body`
/// is the markdown content from the SKILL.md file.
pub fn inject_active_skill_prompts(
    base_prompt: &str,
    active_skills: &[(SkillManifest, String)],
) -> String {
    if active_skills.is_empty() {
        return base_prompt.to_owned();
    }

    let mut skills_block = String::from("\n<active_skills>\n");

    for (manifest, body) in active_skills {
        skills_block.push_str(&format!(
            "<skill name=\"{}\">\n{}\n</skill>\n",
            manifest.name,
            body.trim()
        ));
    }

    skills_block.push_str("</active_skills>");

    // Insert after </identity> if present, otherwise append
    if let Some(pos) = base_prompt.find("</identity>") {
        let insert_at = pos + "</identity>".len();
        let mut result = String::with_capacity(base_prompt.len() + skills_block.len());
        result.push_str(&base_prompt[..insert_at]);
        result.push_str(&skills_block);
        result.push_str(&base_prompt[insert_at..]);
        result
    } else {
        format!("{base_prompt}{skills_block}")
    }
}

// ---------------------------------------------------------------------------
// Convenience: combined prompt builder
// ---------------------------------------------------------------------------

/// Build a skill-enhanced system prompt combining metadata and active bodies.
///
/// 1. Generates Level 1 metadata XML from `available_skills`.
/// 2. Injects Level 2 active skill bodies after `</identity>`.
/// 3. Returns the fully assembled prompt.
pub fn build_skill_enhanced_prompt(
    base_prompt: &str,
    available_skills: &[(SkillManifest, PathBuf)],
    active_skills: &[(SkillManifest, String)],
) -> String {
    // Start with active skill injection (Level 2)
    let with_active = inject_active_skill_prompts(base_prompt, active_skills);

    // Append metadata for all available skills (Level 1)
    let metadata_xml = generate_skill_metadata_xml(available_skills);
    if metadata_xml.is_empty() {
        return with_active;
    }

    format!("{with_active}\n{metadata_xml}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manifest(name: &str, description: &str) -> SkillManifest {
        SkillManifest {
            name: name.to_owned(),
            description: description.to_owned(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        }
    }

    #[test]
    fn metadata_xml_has_name_and_description() {
        let skills = vec![
            (
                make_manifest("web-search", "Search the web"),
                PathBuf::from("/skills/web-search"),
            ),
            (
                make_manifest("code-review", "Review code quality"),
                PathBuf::from("/skills/code-review"),
            ),
        ];

        let xml = generate_skill_metadata_xml(&skills);

        assert!(xml.contains("<available_skills>"));
        assert!(xml.contains("</available_skills>"));
        assert!(xml.contains("name=\"web-search\""));
        assert!(xml.contains("Search the web"));
        assert!(xml.contains("name=\"code-review\""));
        assert!(xml.contains("Review code quality"));
    }

    #[test]
    fn metadata_xml_empty_for_no_skills() {
        let xml = generate_skill_metadata_xml(&[]);
        assert!(xml.is_empty());
    }

    #[test]
    fn active_skills_insert_after_identity() {
        let base = "<identity>\nI am a bot.\n</identity>\n<instructions>\nBe helpful.\n</instructions>";

        let active = vec![(
            make_manifest("web-search", "Search the web"),
            "Use this skill to search the web for information.".to_owned(),
        )];

        let result = inject_active_skill_prompts(base, &active);

        // Skills should appear after </identity>
        let identity_end = result.find("</identity>").unwrap() + "</identity>".len();
        let active_start = result.find("<active_skills>").unwrap();
        assert!(
            active_start >= identity_end,
            "active_skills should appear after </identity>"
        );

        // Instructions should still be present after skills
        assert!(result.contains("<instructions>"));
        assert!(result.contains("Be helpful."));

        // Skill content should be present
        assert!(result.contains("name=\"web-search\""));
        assert!(result.contains("Use this skill to search the web"));
    }

    #[test]
    fn active_skills_appended_without_identity() {
        let base = "You are a helpful bot.";

        let active = vec![(
            make_manifest("greeter", "Greet users"),
            "Always greet warmly.".to_owned(),
        )];

        let result = inject_active_skill_prompts(base, &active);

        assert!(result.starts_with("You are a helpful bot."));
        assert!(result.contains("<active_skills>"));
        assert!(result.contains("name=\"greeter\""));
        assert!(result.contains("Always greet warmly."));
    }

    #[test]
    fn no_skills_returns_base_prompt_unchanged() {
        let base = "<identity>\nI am a bot.\n</identity>";

        let result = inject_active_skill_prompts(base, &[]);
        assert_eq!(result, base);
    }

    #[test]
    fn build_skill_enhanced_prompt_combines_both() {
        let base = "<identity>\nI am a bot.\n</identity>";

        let available = vec![(
            make_manifest("web-search", "Search the web"),
            PathBuf::from("/skills/web-search"),
        )];

        let active = vec![(
            make_manifest("greeter", "Greet users"),
            "Hello there!".to_owned(),
        )];

        let result = build_skill_enhanced_prompt(base, &available, &active);

        // Should have both active skills (Level 2) and metadata (Level 1)
        assert!(result.contains("<active_skills>"));
        assert!(result.contains("name=\"greeter\""));
        assert!(result.contains("<available_skills>"));
        assert!(result.contains("name=\"web-search\""));
    }

    #[test]
    fn build_skill_enhanced_prompt_no_available() {
        let base = "<identity>\nI am a bot.\n</identity>";

        let active = vec![(
            make_manifest("greeter", "Greet users"),
            "Hello there!".to_owned(),
        )];

        let result = build_skill_enhanced_prompt(base, &[], &active);

        assert!(result.contains("<active_skills>"));
        assert!(!result.contains("<available_skills>"));
    }
}
