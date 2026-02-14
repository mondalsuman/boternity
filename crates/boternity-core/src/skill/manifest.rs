//! SKILL.md manifest parsing and validation.
//!
//! Handles the agentskills.io-compatible manifest format: YAML frontmatter
//! delimited by `---` followed by a markdown body containing skill instructions.

use anyhow::{bail, Context};
use boternity_types::skill::{BotSkillsFile, SkillManifest, SkillType};

/// Extract YAML frontmatter and markdown body from a SKILL.md file.
///
/// Content must start with `---\n`, and a closing `\n---\n` (or `\n---` at EOF)
/// separates the YAML from the body.
///
/// Returns `(yaml_str, body_str)` where body has leading whitespace trimmed.
pub fn extract_frontmatter(content: &str) -> anyhow::Result<(&str, &str)> {
    if !content.starts_with("---") {
        bail!("SKILL.md must start with YAML frontmatter delimiter '---'");
    }

    // Skip the opening delimiter line
    let after_open = &content[3..];
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

    // Find the closing delimiter
    let closing_pos = after_open
        .find("\n---")
        .context("SKILL.md missing closing frontmatter delimiter '---'")?;

    let yaml_str = &after_open[..closing_pos];
    let remainder = &after_open[closing_pos + 4..]; // skip "\n---"

    // Body starts after the closing delimiter line
    let body_str = remainder
        .strip_prefix('\n')
        .unwrap_or(remainder)
        .trim_start_matches('\n');

    Ok((yaml_str, body_str))
}

/// Parse a SKILL.md file into a `SkillManifest` and markdown body.
///
/// Splits the content into YAML frontmatter and body, then deserializes
/// the frontmatter into a `SkillManifest`.
pub fn parse_skill_md(content: &str) -> anyhow::Result<(SkillManifest, String)> {
    let (yaml_str, body_str) = extract_frontmatter(content)?;

    let manifest: SkillManifest =
        serde_yaml_ng::from_str(yaml_str).context("Failed to parse SKILL.md YAML frontmatter")?;

    Ok((manifest, body_str.to_owned()))
}

/// Validate a parsed `SkillManifest` for correctness.
///
/// Checks:
/// - `name` is non-empty and matches slug pattern (lowercase alphanumeric + hyphens)
/// - `description` is non-empty
/// - If `metadata.version` is present, it parses as valid semver
/// - If `metadata.skill_type` is `Tool`, `metadata.capabilities` should be present
/// - If `metadata.parents` is present, depth <= 3
/// - If `metadata.conflicts_with` is present, no self-conflict
pub fn validate_manifest(manifest: &SkillManifest) -> anyhow::Result<()> {
    // Name must be non-empty and match slug pattern
    if manifest.name.is_empty() {
        bail!("Skill name must not be empty");
    }

    let is_valid_slug = manifest
        .name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

    if !is_valid_slug {
        bail!(
            "Skill name '{}' must contain only lowercase letters, digits, and hyphens",
            manifest.name
        );
    }

    // Must not start or end with a hyphen
    if manifest.name.starts_with('-') || manifest.name.ends_with('-') {
        bail!(
            "Skill name '{}' must not start or end with a hyphen",
            manifest.name
        );
    }

    // Description must be non-empty
    if manifest.description.is_empty() {
        bail!("Skill description must not be empty");
    }

    // Validate metadata fields if present
    if let Some(ref meta) = manifest.metadata {
        // Version must be valid semver if present
        if let Some(ref version_str) = meta.version {
            version_str
                .parse::<semver::Version>()
                .with_context(|| format!("Invalid semver version '{version_str}'"))?;
        }

        // Tool skills should declare capabilities
        if let Some(SkillType::Tool) = meta.skill_type {
            if meta.capabilities.is_none()
                || meta
                    .capabilities
                    .as_ref()
                    .is_some_and(|caps| caps.is_empty())
            {
                tracing::warn!(
                    skill = %manifest.name,
                    "Tool skill has no capabilities declared; it won't be able to perform any operations"
                );
            }
        }

        // Parent chain depth limit
        if let Some(ref parents) = meta.parents {
            if parents.len() > 3 {
                tracing::warn!(
                    skill = %manifest.name,
                    depth = parents.len(),
                    "Skill parent chain exceeds recommended depth of 3"
                );
            }
        }

        // No self-conflict
        if let Some(ref conflicts) = meta.conflicts_with {
            if conflicts.iter().any(|c| c == &manifest.name) {
                bail!(
                    "Skill '{}' lists itself in conflicts_with",
                    manifest.name
                );
            }
        }
    }

    Ok(())
}

/// Parse a per-bot `skills.toml` configuration file.
pub fn parse_bot_skills_config(content: &str) -> anyhow::Result<BotSkillsFile> {
    toml::from_str(content).context("Failed to parse skills.toml")
}

/// Serialize a per-bot skills configuration to TOML string.
pub fn serialize_bot_skills_config(config: &BotSkillsFile) -> anyhow::Result<String> {
    toml::to_string_pretty(config).context("Failed to serialize skills.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{BotSkillConfig, Capability, TrustTier};
    use std::collections::HashMap;

    const FULL_SKILL_MD: &str = r#"---
name: web-search
description: Search the web for information
license: MIT
compatibility: ">=0.1.0"
metadata:
  author: boternity
  version: "1.2.0"
  skill-type: tool
  capabilities:
    - http_get
    - read_env
  dependencies:
    - url-parser
  conflicts-with:
    - web-scrape
  trust-tier: verified
  parents:
    - base-search
  secrets:
    - SEARCH_API_KEY
  categories:
    - search
    - web
allowed-tools: web_search
---

# Web Search Skill

This skill allows a bot to search the web for information.

## Usage

Ask the bot to search for anything.
"#;

    const MINIMAL_SKILL_MD: &str = r#"---
name: hello-world
description: A simple greeting skill
---

Say hello to the user.
"#;

    #[test]
    fn parse_full_skill_md() {
        let (manifest, body) = parse_skill_md(FULL_SKILL_MD).unwrap();

        assert_eq!(manifest.name, "web-search");
        assert_eq!(manifest.description, "Search the web for information");
        assert_eq!(manifest.license.as_deref(), Some("MIT"));
        assert_eq!(manifest.compatibility.as_deref(), Some(">=0.1.0"));
        assert_eq!(manifest.allowed_tools.as_deref(), Some("web_search"));

        let meta = manifest.metadata.as_ref().unwrap();
        assert_eq!(meta.author.as_deref(), Some("boternity"));
        assert_eq!(meta.version.as_deref(), Some("1.2.0"));
        assert!(matches!(meta.skill_type, Some(SkillType::Tool)));
        assert_eq!(
            meta.capabilities.as_ref().unwrap(),
            &[Capability::HttpGet, Capability::ReadEnv]
        );
        assert_eq!(
            meta.dependencies.as_ref().unwrap(),
            &["url-parser".to_owned()]
        );
        assert_eq!(
            meta.conflicts_with.as_ref().unwrap(),
            &["web-scrape".to_owned()]
        );
        assert_eq!(meta.trust_tier, Some(TrustTier::Verified));
        assert_eq!(
            meta.parents.as_ref().unwrap(),
            &["base-search".to_owned()]
        );
        assert_eq!(
            meta.secrets.as_ref().unwrap(),
            &["SEARCH_API_KEY".to_owned()]
        );
        assert_eq!(
            meta.categories.as_ref().unwrap(),
            &["search".to_owned(), "web".to_owned()]
        );

        assert!(body.contains("# Web Search Skill"));
        assert!(body.contains("Ask the bot to search for anything."));

        // Validate the parsed manifest
        validate_manifest(&manifest).unwrap();
    }

    #[test]
    fn parse_minimal_skill_md() {
        let (manifest, body) = parse_skill_md(MINIMAL_SKILL_MD).unwrap();

        assert_eq!(manifest.name, "hello-world");
        assert_eq!(manifest.description, "A simple greeting skill");
        assert!(manifest.license.is_none());
        assert!(manifest.metadata.is_none());

        assert!(body.contains("Say hello to the user."));

        validate_manifest(&manifest).unwrap();
    }

    #[test]
    fn reject_missing_frontmatter() {
        let content = "# No Frontmatter\n\nJust a markdown file.";
        let result = parse_skill_md(content);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must start with YAML frontmatter")
        );
    }

    #[test]
    fn reject_missing_closing_delimiter() {
        let content = "---\nname: broken\ndescription: no closing\n";
        let result = parse_skill_md(content);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing closing frontmatter")
        );
    }

    #[test]
    fn validate_invalid_name_spaces() {
        let manifest = SkillManifest {
            name: "my skill".to_owned(),
            description: "Has spaces".to_owned(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };
        let result = validate_manifest(&manifest);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("lowercase letters, digits, and hyphens")
        );
    }

    #[test]
    fn validate_invalid_name_uppercase() {
        let manifest = SkillManifest {
            name: "MySkill".to_owned(),
            description: "Has uppercase".to_owned(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };
        let result = validate_manifest(&manifest);
        assert!(result.is_err());
    }

    #[test]
    fn validate_empty_name() {
        let manifest = SkillManifest {
            name: String::new(),
            description: "Has description".to_owned(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };
        let result = validate_manifest(&manifest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must not be empty"));
    }

    #[test]
    fn validate_empty_description() {
        let manifest = SkillManifest {
            name: "good-name".to_owned(),
            description: String::new(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };
        let result = validate_manifest(&manifest);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("description must not be empty")
        );
    }

    #[test]
    fn validate_self_conflict() {
        use boternity_types::skill::SkillMetadata;

        let manifest = SkillManifest {
            name: "my-skill".to_owned(),
            description: "A skill".to_owned(),
            license: None,
            compatibility: None,
            metadata: Some(SkillMetadata {
                author: None,
                version: None,
                skill_type: None,
                capabilities: None,
                dependencies: None,
                conflicts_with: Some(vec!["my-skill".to_owned()]),
                trust_tier: None,
                parents: None,
                secrets: None,
                categories: None,
            }),
            allowed_tools: None,
        };
        let result = validate_manifest(&manifest);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("lists itself in conflicts_with")
        );
    }

    #[test]
    fn validate_invalid_semver() {
        use boternity_types::skill::SkillMetadata;

        let manifest = SkillManifest {
            name: "my-skill".to_owned(),
            description: "A skill".to_owned(),
            license: None,
            compatibility: None,
            metadata: Some(SkillMetadata {
                author: None,
                version: Some("not-a-version".to_owned()),
                skill_type: None,
                capabilities: None,
                dependencies: None,
                conflicts_with: None,
                trust_tier: None,
                parents: None,
                secrets: None,
                categories: None,
            }),
            allowed_tools: None,
        };
        let result = validate_manifest(&manifest);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid semver version")
        );
    }

    #[test]
    fn bot_skills_config_round_trip() {
        let mut skills = HashMap::new();
        skills.insert(
            "web-search".to_owned(),
            BotSkillConfig {
                skill_name: "web-search".to_owned(),
                enabled: true,
                trust_tier: Some(TrustTier::Verified),
                version: Some("1.2.0".to_owned()),
                overrides: HashMap::new(),
                capabilities: Some(vec![Capability::HttpGet]),
            },
        );
        skills.insert(
            "hello-world".to_owned(),
            BotSkillConfig {
                skill_name: "hello-world".to_owned(),
                enabled: false,
                trust_tier: None,
                version: None,
                overrides: HashMap::new(),
                capabilities: None,
            },
        );

        let config = BotSkillsFile { skills };
        let toml_str = serialize_bot_skills_config(&config).unwrap();
        let parsed = parse_bot_skills_config(&toml_str).unwrap();

        assert_eq!(parsed.skills.len(), 2);
        assert!(parsed.skills["web-search"].enabled);
        assert!(!parsed.skills["hello-world"].enabled);
        assert_eq!(
            parsed.skills["web-search"].trust_tier,
            Some(TrustTier::Verified)
        );
        assert_eq!(
            parsed.skills["web-search"].capabilities,
            Some(vec![Capability::HttpGet])
        );
    }
}
