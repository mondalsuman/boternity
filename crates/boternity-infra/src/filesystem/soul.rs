//! SOUL.md file operations.
//!
//! Handles reading and parsing SOUL.md files with YAML frontmatter.
//! Format:
//! ```text
//! ---
//! name: Luna
//! traits: [curious, empathetic, analytical]
//! tone: warm and conversational
//! ---
//! # Luna
//! Personality content here...
//! ```

use boternity_types::soul::SoulFrontmatter;

/// Parse SOUL.md content into frontmatter and body.
///
/// Returns `(frontmatter_yaml, body_markdown)`.
/// The frontmatter is the text between the first pair of `---` delimiters.
pub fn parse_soul_content(content: &str) -> Option<(&str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing --- delimiter (skip the opening one)
    let after_opening = &trimmed[3..];
    let closing_pos = after_opening.find("\n---")?;
    let frontmatter = after_opening[..closing_pos].trim();
    let body = after_opening[closing_pos + 4..].trim_start_matches(['\n', '\r']);

    Some((frontmatter, body))
}

/// Parse the YAML frontmatter of a SOUL.md file into a `SoulFrontmatter` struct.
pub fn parse_soul_frontmatter(content: &str) -> Option<SoulFrontmatter> {
    let (yaml_str, _body) = parse_soul_content(content)?;

    // Simple YAML parsing without a full YAML library:
    // We parse the known fields: name, traits (list), tone
    let mut name = None;
    let mut traits = Vec::new();
    let mut tone = None;
    let mut in_traits = false;

    for line in yaml_str.lines() {
        let line = line.trim();
        if line.starts_with("name:") {
            name = Some(line.trim_start_matches("name:").trim().to_string());
            in_traits = false;
        } else if line.starts_with("tone:") {
            tone = Some(line.trim_start_matches("tone:").trim().to_string());
            in_traits = false;
        } else if line.starts_with("traits:") {
            in_traits = true;
            // Check for inline array: traits: [a, b, c]
            let rest = line.trim_start_matches("traits:").trim();
            if rest.starts_with('[') && rest.ends_with(']') {
                let inner = &rest[1..rest.len() - 1];
                traits = inner.split(',').map(|s| s.trim().to_string()).collect();
                in_traits = false;
            }
        } else if in_traits && line.starts_with("- ") {
            traits.push(line.trim_start_matches("- ").trim().to_string());
        } else if in_traits && !line.starts_with('-') && !line.is_empty() {
            in_traits = false;
        }
    }

    Some(SoulFrontmatter {
        name: name?,
        traits,
        tone: tone.unwrap_or_default(),
    })
}

/// Compose a SOUL.md file from frontmatter and body.
pub fn compose_soul_content(frontmatter: &SoulFrontmatter, body: &str) -> String {
    let mut result = String::from("---\n");
    result.push_str(&format!("name: {}\n", frontmatter.name));
    result.push_str("traits:\n");
    for t in &frontmatter.traits {
        result.push_str(&format!("  - {t}\n"));
    }
    result.push_str(&format!("tone: {}\n", frontmatter.tone));
    result.push_str("---\n\n");
    result.push_str(body);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SOUL: &str = r#"---
name: Luna
traits:
  - curious
  - empathetic
  - analytical
tone: warm and conversational
---

# Luna

You are Luna, a curious and empathetic individual.
"#;

    #[test]
    fn test_parse_soul_content_splits_correctly() {
        let (frontmatter, body) = parse_soul_content(SAMPLE_SOUL).unwrap();
        assert!(frontmatter.contains("name: Luna"));
        assert!(frontmatter.contains("traits:"));
        assert!(body.starts_with("# Luna"));
    }

    #[test]
    fn test_parse_soul_frontmatter() {
        let fm = parse_soul_frontmatter(SAMPLE_SOUL).unwrap();
        assert_eq!(fm.name, "Luna");
        assert_eq!(fm.traits, vec!["curious", "empathetic", "analytical"]);
        assert_eq!(fm.tone, "warm and conversational");
    }

    #[test]
    fn test_parse_soul_frontmatter_inline_traits() {
        let content = "---\nname: Bot\ntraits: [fast, smart]\ntone: direct\n---\nBody";
        let fm = parse_soul_frontmatter(content).unwrap();
        assert_eq!(fm.traits, vec!["fast", "smart"]);
    }

    #[test]
    fn test_parse_soul_no_frontmatter() {
        assert!(parse_soul_content("Just a regular file").is_none());
    }

    #[test]
    fn test_compose_soul_content() {
        let fm = SoulFrontmatter {
            name: "Luna".to_string(),
            traits: vec!["curious".to_string(), "warm".to_string()],
            tone: "friendly".to_string(),
        };
        let composed = compose_soul_content(&fm, "# Luna\nHello!");
        assert!(composed.starts_with("---\n"));
        assert!(composed.contains("name: Luna"));
        assert!(composed.contains("  - curious"));
        assert!(composed.contains("  - warm"));
        assert!(composed.contains("tone: friendly"));
        assert!(composed.contains("# Luna\nHello!"));
    }

    #[test]
    fn test_roundtrip_parse_compose() {
        let fm = SoulFrontmatter {
            name: "TestBot".to_string(),
            traits: vec!["a".to_string(), "b".to_string()],
            tone: "casual".to_string(),
        };
        let body = "# TestBot\nContent here.";
        let composed = compose_soul_content(&fm, body);
        let parsed = parse_soul_frontmatter(&composed).unwrap();
        assert_eq!(parsed.name, fm.name);
        assert_eq!(parsed.traits, fm.traits);
        assert_eq!(parsed.tone, fm.tone);
    }
}
