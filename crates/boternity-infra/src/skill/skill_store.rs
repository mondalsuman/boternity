//! Filesystem-based skill storage.
//!
//! Manages skills on disk at `~/.boternity/skills/`. Each skill occupies a
//! directory containing `SKILL.md`, optional `.boternity-meta.toml`, and
//! optional `skill.wasm`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use boternity_core::skill::manifest::{
    parse_bot_skills_config, parse_skill_md, serialize_bot_skills_config,
};
use boternity_types::skill::{
    BotSkillsFile, InstalledSkill, SkillMeta, SkillSource,
};

/// Filesystem-based skill store managing skills at a configurable base directory.
///
/// Default layout:
/// ```text
/// {base_dir}/skills/{skill-name}/
///   SKILL.md
///   .boternity-meta.toml
///   skill.wasm
/// ```
#[derive(Debug, Clone)]
pub struct SkillStore {
    base_dir: PathBuf,
}

impl SkillStore {
    /// Create a new skill store rooted at `base_dir`.
    ///
    /// The skills directory will be `{base_dir}/skills/`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Return the path to the skills directory.
    fn skills_dir(&self) -> PathBuf {
        self.base_dir.join("skills")
    }

    /// Resolve the full path to a skill directory.
    pub fn resolve_skill_path(&self, name: &str) -> PathBuf {
        self.skills_dir().join(name)
    }

    /// Check whether a skill exists on disk.
    pub fn skill_exists(&self, name: &str) -> bool {
        self.resolve_skill_path(name).join("SKILL.md").exists()
    }

    /// List all installed skills.
    ///
    /// Scans the skills directory for subdirectories containing SKILL.md files,
    /// parses each one, and returns the full list.
    pub fn list_skills(&self) -> anyhow::Result<Vec<InstalledSkill>> {
        let skills_dir = self.skills_dir();
        if !skills_dir.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();
        let entries = std::fs::read_dir(&skills_dir)
            .with_context(|| format!("Failed to read skills directory: {}", skills_dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_md_path = path.join("SKILL.md");
            if !skill_md_path.exists() {
                continue;
            }

            // Try to load the skill; skip on error (corrupted skill)
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            match self.get_skill(name) {
                Ok(skill) => skills.push(skill),
                Err(e) => {
                    tracing::warn!(skill = %name, error = %e, "Skipping corrupted skill");
                }
            }
        }

        Ok(skills)
    }

    /// Load a single skill by name.
    ///
    /// Reads SKILL.md, parses it, reads optional `.boternity-meta.toml`, and
    /// checks for `skill.wasm`.
    pub fn get_skill(&self, name: &str) -> anyhow::Result<InstalledSkill> {
        let skill_dir = self.resolve_skill_path(name);
        let skill_md_path = skill_dir.join("SKILL.md");

        if !skill_md_path.exists() {
            bail!("Skill '{}' not found at {}", name, skill_dir.display());
        }

        let content = std::fs::read_to_string(&skill_md_path)
            .with_context(|| format!("Failed to read {}", skill_md_path.display()))?;

        let (manifest, body) = parse_skill_md(&content)?;

        // Read source metadata from .boternity-meta.toml if present
        let meta_path = skill_dir.join(".boternity-meta.toml");
        let source = if meta_path.exists() {
            let meta_content = std::fs::read_to_string(&meta_path)
                .with_context(|| format!("Failed to read {}", meta_path.display()))?;
            let meta: SkillMeta = toml::from_str(&meta_content)
                .with_context(|| format!("Failed to parse {}", meta_path.display()))?;
            meta.source
        } else {
            SkillSource::Local
        };

        // Check for WASM binary
        let wasm_path = skill_dir.join("skill.wasm");
        let wasm_path = if wasm_path.exists() {
            Some(wasm_path)
        } else {
            None
        };

        Ok(InstalledSkill {
            manifest,
            body,
            source,
            install_path: skill_dir,
            wasm_path,
        })
    }

    /// Install a skill to disk.
    ///
    /// Creates the skill directory, writes SKILL.md, optional `.boternity-meta.toml`,
    /// and optional `skill.wasm`. Returns the path to the skill directory.
    pub fn install_skill(
        &self,
        name: &str,
        content: &str,
        meta: Option<SkillMeta>,
        wasm_bytes: Option<&[u8]>,
    ) -> anyhow::Result<PathBuf> {
        let skill_dir = self.resolve_skill_path(name);
        std::fs::create_dir_all(&skill_dir)
            .with_context(|| format!("Failed to create skill directory: {}", skill_dir.display()))?;

        // Write SKILL.md
        let skill_md_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md_path, content)
            .with_context(|| format!("Failed to write {}", skill_md_path.display()))?;

        // Write .boternity-meta.toml if provided
        if let Some(ref meta) = meta {
            let meta_path = skill_dir.join(".boternity-meta.toml");
            let meta_content = toml::to_string_pretty(meta)
                .context("Failed to serialize skill metadata")?;
            std::fs::write(&meta_path, meta_content)
                .with_context(|| format!("Failed to write {}", meta_path.display()))?;
        }

        // Write skill.wasm if provided
        if let Some(wasm) = wasm_bytes {
            let wasm_path = skill_dir.join("skill.wasm");
            std::fs::write(&wasm_path, wasm)
                .with_context(|| format!("Failed to write {}", wasm_path.display()))?;
        }

        Ok(skill_dir)
    }

    /// Remove a skill from disk.
    ///
    /// Deletes the entire skill directory.
    pub fn remove_skill(&self, name: &str) -> anyhow::Result<()> {
        let skill_dir = self.resolve_skill_path(name);

        if !skill_dir.exists() {
            bail!("Skill '{}' not found at {}", name, skill_dir.display());
        }

        std::fs::remove_dir_all(&skill_dir)
            .with_context(|| format!("Failed to remove skill directory: {}", skill_dir.display()))?;

        Ok(())
    }

    /// Read the per-bot skills configuration from `skills.toml` in the bot directory.
    ///
    /// Returns an empty configuration if the file does not exist.
    pub fn get_bot_skills_config(&self, bot_dir: &Path) -> anyhow::Result<BotSkillsFile> {
        let config_path = bot_dir.join("skills.toml");

        if !config_path.exists() {
            return Ok(BotSkillsFile {
                skills: std::collections::HashMap::new(),
            });
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;

        parse_bot_skills_config(&content)
    }

    /// Save the per-bot skills configuration to `skills.toml` in the bot directory.
    pub fn save_bot_skills_config(
        &self,
        bot_dir: &Path,
        config: &BotSkillsFile,
    ) -> anyhow::Result<()> {
        let config_path = bot_dir.join("skills.toml");

        std::fs::create_dir_all(bot_dir)
            .with_context(|| format!("Failed to create bot directory: {}", bot_dir.display()))?;

        let content = serialize_bot_skills_config(config)?;
        std::fs::write(&config_path, &content)
            .with_context(|| format!("Failed to write {}", config_path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{BotSkillConfig, Capability, TrustTier};
    use std::collections::HashMap;

    const TEST_SKILL_MD: &str = "\
---
name: test-skill
description: A test skill
metadata:
  author: tester
  version: \"1.0.0\"
  skill-type: tool
  capabilities:
    - http_get
---

This is the test skill body.
";

    fn make_store(tmpdir: &tempfile::TempDir) -> SkillStore {
        SkillStore::new(tmpdir.path().to_path_buf())
    }

    #[test]
    fn install_and_list_round_trip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);

        // Install a skill
        let path = store
            .install_skill("test-skill", TEST_SKILL_MD, None, None)
            .unwrap();
        assert!(path.exists());
        assert!(path.join("SKILL.md").exists());

        // List skills
        let skills = store.list_skills().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].manifest.name, "test-skill");
        assert_eq!(skills[0].manifest.description, "A test skill");
        assert!(skills[0].body.contains("test skill body"));
        assert!(matches!(skills[0].source, SkillSource::Local));
    }

    #[test]
    fn install_with_meta_and_wasm() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);

        let meta = SkillMeta {
            source: SkillSource::Registry {
                registry_name: "agentskills.io".to_owned(),
                repo: "test-org/test-skill".to_owned(),
                path: "/skills/test-skill".to_owned(),
            },
            installed_at: chrono::Utc::now(),
            version: "1.0.0".parse().unwrap(),
            checksum: "abc123".to_owned(),
            trust_tier: TrustTier::Verified,
        };

        let wasm_bytes = b"fake wasm binary";

        store
            .install_skill("test-skill", TEST_SKILL_MD, Some(meta), Some(wasm_bytes))
            .unwrap();

        let skill = store.get_skill("test-skill").unwrap();
        assert!(matches!(skill.source, SkillSource::Registry { .. }));
        assert!(skill.wasm_path.is_some());
        assert!(skill.wasm_path.unwrap().exists());
    }

    #[test]
    fn get_nonexistent_skill_returns_error() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);

        let result = store.get_skill("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn remove_skill_deletes_directory() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);

        store
            .install_skill("test-skill", TEST_SKILL_MD, None, None)
            .unwrap();
        assert!(store.skill_exists("test-skill"));

        store.remove_skill("test-skill").unwrap();
        assert!(!store.skill_exists("test-skill"));

        // Double remove should error
        let result = store.remove_skill("test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn bot_skills_config_read_write_round_trip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);
        let bot_dir = tmpdir.path().join("bots").join("test-bot");

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

        let config = BotSkillsFile { skills };
        store.save_bot_skills_config(&bot_dir, &config).unwrap();

        let loaded = store.get_bot_skills_config(&bot_dir).unwrap();
        assert_eq!(loaded.skills.len(), 1);
        assert!(loaded.skills["web-search"].enabled);
        assert_eq!(
            loaded.skills["web-search"].trust_tier,
            Some(TrustTier::Verified)
        );
    }

    #[test]
    fn get_bot_skills_config_missing_file_returns_empty() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);
        let bot_dir = tmpdir.path().join("bots").join("no-such-bot");

        let config = store.get_bot_skills_config(&bot_dir).unwrap();
        assert!(config.skills.is_empty());
    }

    #[test]
    fn skill_exists_check() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);

        assert!(!store.skill_exists("test-skill"));

        store
            .install_skill("test-skill", TEST_SKILL_MD, None, None)
            .unwrap();
        assert!(store.skill_exists("test-skill"));
    }

    #[test]
    fn resolve_skill_path_returns_correct_path() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = make_store(&tmpdir);

        let path = store.resolve_skill_path("my-skill");
        assert_eq!(path, tmpdir.path().join("skills").join("my-skill"));
    }
}
