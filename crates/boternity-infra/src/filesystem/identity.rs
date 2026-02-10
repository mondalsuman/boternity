//! IDENTITY.md file operations.
//!
//! Handles reading and parsing IDENTITY.md files with YAML frontmatter.
//! Format:
//! ```text
//! ---
//! display_name: Luna
//! category: assistant
//! model: claude-sonnet-4-20250514
//! provider: anthropic
//! temperature: 0.7
//! max_tokens: 4096
//! ---
//! # Luna - Identity Configuration
//! ...
//! ```

use boternity_types::bot::{BotCategory, BotId};
use boternity_types::identity::Identity;

/// Parsed IDENTITY.md frontmatter fields.
#[derive(Debug, Clone)]
pub struct IdentityFrontmatter {
    pub display_name: String,
    pub category: String,
    pub model: String,
    pub provider: String,
    pub temperature: f64,
    pub max_tokens: i32,
}

/// Parse the IDENTITY.md content into frontmatter fields.
pub fn parse_identity_content(content: &str) -> Option<(&str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_opening = &trimmed[3..];
    let closing_pos = after_opening.find("\n---")?;
    let frontmatter = after_opening[..closing_pos].trim();
    let body = after_opening[closing_pos + 4..].trim_start_matches(['\n', '\r']);

    Some((frontmatter, body))
}

/// Parse IDENTITY.md frontmatter into structured data.
pub fn parse_identity_frontmatter(content: &str) -> Option<IdentityFrontmatter> {
    let (yaml_str, _body) = parse_identity_content(content)?;

    let mut display_name = None;
    let mut category = None;
    let mut model = None;
    let mut provider = None;
    let mut temperature = None;
    let mut max_tokens = None;

    for line in yaml_str.lines() {
        let line = line.trim();
        if line.starts_with("display_name:") {
            display_name = Some(line.trim_start_matches("display_name:").trim().to_string());
        } else if line.starts_with("category:") {
            category = Some(line.trim_start_matches("category:").trim().to_string());
        } else if line.starts_with("model:") {
            model = Some(line.trim_start_matches("model:").trim().to_string());
        } else if line.starts_with("provider:") {
            provider = Some(line.trim_start_matches("provider:").trim().to_string());
        } else if line.starts_with("temperature:") {
            temperature = line
                .trim_start_matches("temperature:")
                .trim()
                .parse::<f64>()
                .ok();
        } else if line.starts_with("max_tokens:") {
            max_tokens = line
                .trim_start_matches("max_tokens:")
                .trim()
                .parse::<i32>()
                .ok();
        }
    }

    Some(IdentityFrontmatter {
        display_name: display_name?,
        category: category.unwrap_or_else(|| "assistant".to_string()),
        model: model.unwrap_or_else(|| Identity::DEFAULT_MODEL.to_string()),
        provider: provider.unwrap_or_else(|| Identity::DEFAULT_PROVIDER.to_string()),
        temperature: temperature.unwrap_or(Identity::DEFAULT_TEMPERATURE),
        max_tokens: max_tokens.unwrap_or(Identity::DEFAULT_MAX_TOKENS),
    })
}

/// Convert parsed identity frontmatter to a domain `Identity` struct.
pub fn frontmatter_to_identity(
    bot_id: BotId,
    fm: &IdentityFrontmatter,
) -> Identity {
    let category: BotCategory = fm.category.parse().unwrap_or_default();
    Identity {
        bot_id,
        display_name: fm.display_name.clone(),
        avatar: None,
        accent_color: None,
        emoji: None,
        model: fm.model.clone(),
        provider: fm.provider.clone(),
        temperature: fm.temperature,
        max_tokens: fm.max_tokens,
        category,
        tags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_IDENTITY: &str = r#"---
display_name: Luna
category: assistant
model: claude-sonnet-4-20250514
provider: anthropic
temperature: 0.7
max_tokens: 4096
---

# Luna - Identity Configuration
"#;

    #[test]
    fn test_parse_identity_content() {
        let (fm, body) = parse_identity_content(SAMPLE_IDENTITY).unwrap();
        assert!(fm.contains("display_name: Luna"));
        assert!(body.contains("Identity Configuration"));
    }

    #[test]
    fn test_parse_identity_frontmatter() {
        let fm = parse_identity_frontmatter(SAMPLE_IDENTITY).unwrap();
        assert_eq!(fm.display_name, "Luna");
        assert_eq!(fm.category, "assistant");
        assert_eq!(fm.model, "claude-sonnet-4-20250514");
        assert_eq!(fm.provider, "anthropic");
        assert!((fm.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(fm.max_tokens, 4096);
    }

    #[test]
    fn test_frontmatter_to_identity() {
        let fm = IdentityFrontmatter {
            display_name: "Luna".to_string(),
            category: "research".to_string(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            temperature: 0.5,
            max_tokens: 2048,
        };
        let identity = frontmatter_to_identity(BotId::new(), &fm);
        assert_eq!(identity.display_name, "Luna");
        assert_eq!(identity.category, BotCategory::Research);
        assert_eq!(identity.model, "gpt-4");
        assert_eq!(identity.max_tokens, 2048);
    }

    #[test]
    fn test_parse_identity_with_defaults() {
        let content = "---\ndisplay_name: MinBot\n---\nBody";
        let fm = parse_identity_frontmatter(content).unwrap();
        assert_eq!(fm.display_name, "MinBot");
        assert_eq!(fm.category, "assistant"); // default
        assert_eq!(fm.model, Identity::DEFAULT_MODEL); // default
    }
}
