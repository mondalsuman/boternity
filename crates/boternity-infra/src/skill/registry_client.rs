//! Registry clients for skill discovery.
//!
//! Implements `SkillRegistry` for GitHub repositories and the skills.sh API.
//! Each client handles fetching, parsing, caching, and searching skills from
//! its respective source.

use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use boternity_core::skill::manifest::parse_skill_md;
use boternity_core::skill::registry::{
    DiscoveredSkill, RegistryConfig, RegistryType, SkillIndex, SkillRegistry,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Cache freshness duration: 24 hours.
const CACHE_FRESHNESS_SECS: i64 = 24 * 60 * 60;

/// GitHub raw content base URL.
const GITHUB_RAW_BASE: &str = "https://raw.githubusercontent.com";

/// GitHub API base URL.
const GITHUB_API_BASE: &str = "https://api.github.com";

/// skills.sh API base URL.
const SKILLS_SH_BASE: &str = "https://skills.sh/api";

// ---------------------------------------------------------------------------
// Default registry configurations
// ---------------------------------------------------------------------------

/// Returns the default set of registry configurations.
///
/// Includes:
/// 1. ComposioHQ/awesome-claude-skills on GitHub
/// 2. anthropics/skills on GitHub
/// 3. skills.sh aggregation API
pub fn default_registry_configs() -> Vec<RegistryConfig> {
    vec![
        RegistryConfig {
            name: "composiohq".to_string(),
            registry_type: RegistryType::GitHub {
                owner: "ComposioHQ".to_string(),
                repo: "awesome-claude-skills".to_string(),
            },
            enabled: true,
        },
        RegistryConfig {
            name: "anthropics".to_string(),
            registry_type: RegistryType::GitHub {
                owner: "anthropics".to_string(),
                repo: "skills".to_string(),
            },
            enabled: true,
        },
        RegistryConfig {
            name: "skills-sh".to_string(),
            registry_type: RegistryType::SkillsSh,
            enabled: true,
        },
    ]
}

// ---------------------------------------------------------------------------
// GitHub registry client
// ---------------------------------------------------------------------------

/// Registry client for GitHub repositories containing skills.
///
/// Uses the GitHub Trees API to scan a repository for directories containing
/// SKILL.md files, then fetches manifests from raw.githubusercontent.com.
///
/// The index is cached locally at `{cache_dir}/{owner}-{repo}-index.json`
/// with a 24-hour freshness window.
pub struct GitHubRegistryClient {
    owner: String,
    repo: String,
    registry_name: String,
    cache_dir: PathBuf,
    http: reqwest::Client,
}

impl GitHubRegistryClient {
    /// Create a new GitHub registry client.
    ///
    /// `cache_dir` is the directory where the index cache file will be stored.
    pub fn new(owner: String, repo: String, registry_name: String, cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("boternity-skill-registry/0.1")
            .build()
            .unwrap_or_default();

        Self {
            owner,
            repo,
            registry_name,
            cache_dir,
            http,
        }
    }

    /// Path to the cached index file.
    fn cache_path(&self) -> PathBuf {
        self.cache_dir
            .join(format!("{}-{}-index.json", self.owner, self.repo))
    }

    /// Load the cached index if it exists and is fresh (< 24 hours old).
    fn load_cache(&self) -> Option<SkillIndex> {
        let path = self.cache_path();
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let index: SkillIndex = serde_json::from_str(&content).ok()?;

        let age = chrono::Utc::now()
            .signed_duration_since(index.last_updated)
            .num_seconds();

        if age < CACHE_FRESHNESS_SECS {
            debug!(
                registry = %self.registry_name,
                age_secs = age,
                skills = index.skills.len(),
                "Using cached skill index"
            );
            Some(index)
        } else {
            debug!(
                registry = %self.registry_name,
                age_secs = age,
                "Cache expired, will refresh"
            );
            None
        }
    }

    /// Save the index to the cache file.
    fn save_cache(&self, index: &SkillIndex) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("Failed to create cache dir: {}", self.cache_dir.display()))?;

        let content = serde_json::to_string_pretty(index)
            .context("Failed to serialize skill index")?;

        std::fs::write(self.cache_path(), content)
            .with_context(|| format!("Failed to write cache: {}", self.cache_path().display()))?;

        Ok(())
    }

    /// Fetch the skill index from GitHub, scanning the repo tree.
    ///
    /// Uses the Git Trees API with `?recursive=1` to get all paths, then
    /// identifies directories containing SKILL.md files and fetches each.
    async fn fetch_index(&self) -> anyhow::Result<SkillIndex> {
        debug!(
            owner = %self.owner,
            repo = %self.repo,
            "Fetching skill index from GitHub"
        );

        // Fetch the repo tree
        let tree_url = format!(
            "{}/repos/{}/{}/git/trees/main?recursive=1",
            GITHUB_API_BASE, self.owner, self.repo
        );

        let tree_response: GitTreeResponse = self
            .http
            .get(&tree_url)
            .send()
            .await
            .context("Failed to fetch GitHub tree")?
            .error_for_status()
            .context("GitHub tree API returned error")?
            .json()
            .await
            .context("Failed to parse GitHub tree response")?;

        // Find directories that contain SKILL.md
        let skill_paths: Vec<String> = tree_response
            .tree
            .iter()
            .filter(|entry| {
                entry.entry_type == "blob"
                    && entry.path.ends_with("/SKILL.md")
            })
            .map(|entry| {
                // Extract parent directory path
                entry
                    .path
                    .rsplit_once('/')
                    .map(|(parent, _)| parent.to_string())
                    .unwrap_or_default()
            })
            .filter(|p| !p.is_empty())
            .collect();

        debug!(
            count = skill_paths.len(),
            "Found skill directories in repository"
        );

        // Fetch each SKILL.md and parse it
        let mut skills = Vec::new();
        for dir_path in &skill_paths {
            let raw_url = format!(
                "{}/{}/{}/main/{}/SKILL.md",
                GITHUB_RAW_BASE, self.owner, self.repo, dir_path
            );

            match self.fetch_and_parse_skill(&raw_url, dir_path).await {
                Ok(skill) => skills.push(skill),
                Err(e) => {
                    warn!(
                        path = %dir_path,
                        error = %e,
                        "Failed to parse skill, skipping"
                    );
                }
            }
        }

        let index = SkillIndex {
            skills,
            last_updated: chrono::Utc::now(),
            source: self.registry_name.clone(),
        };

        // Cache the index
        if let Err(e) = self.save_cache(&index) {
            warn!(error = %e, "Failed to save skill index cache");
        }

        Ok(index)
    }

    /// Fetch a single SKILL.md from raw.githubusercontent.com and parse it.
    async fn fetch_and_parse_skill(
        &self,
        raw_url: &str,
        dir_path: &str,
    ) -> anyhow::Result<DiscoveredSkill> {
        let content = self
            .http
            .get(raw_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch {raw_url}"))?
            .error_for_status()
            .with_context(|| format!("HTTP error fetching {raw_url}"))?
            .text()
            .await
            .with_context(|| format!("Failed to read body from {raw_url}"))?;

        let (manifest, _body) = parse_skill_md(&content)
            .with_context(|| format!("Failed to parse SKILL.md at {dir_path}"))?;

        let categories = manifest
            .metadata
            .as_ref()
            .and_then(|m| m.categories.clone())
            .unwrap_or_default();

        Ok(DiscoveredSkill {
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            source: self.registry_name.clone(),
            path: dir_path.to_string(),
            manifest,
            install_count: None,
            categories,
        })
    }

    /// Get the index (from cache or fresh fetch).
    async fn get_index(&self) -> anyhow::Result<SkillIndex> {
        if let Some(cached) = self.load_cache() {
            return Ok(cached);
        }
        self.fetch_index().await
    }
}

impl SkillRegistry for GitHubRegistryClient {
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<DiscoveredSkill>> {
        let index = self.get_index().await?;
        let query_lower = query.to_lowercase();

        let mut results: Vec<DiscoveredSkill> = index
            .skills
            .into_iter()
            .filter(|skill| {
                skill.name.to_lowercase().contains(&query_lower)
                    || skill.description.to_lowercase().contains(&query_lower)
                    || skill
                        .categories
                        .iter()
                        .any(|c| c.to_lowercase().contains(&query_lower))
            })
            .collect();

        results.truncate(limit);
        Ok(results)
    }

    async fn list(
        &self,
        offset: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<DiscoveredSkill>> {
        let index = self.get_index().await?;

        let results: Vec<DiscoveredSkill> = index
            .skills
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        Ok(results)
    }

    async fn fetch_skill(
        &self,
        skill: &DiscoveredSkill,
    ) -> anyhow::Result<(String, Option<Vec<u8>>)> {
        let raw_url = format!(
            "{}/{}/{}/main/{}/SKILL.md",
            GITHUB_RAW_BASE, self.owner, self.repo, skill.path
        );

        let content = self
            .http
            .get(&raw_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        // Check for skill.wasm
        let wasm_url = format!(
            "{}/{}/{}/main/{}/skill.wasm",
            GITHUB_RAW_BASE, self.owner, self.repo, skill.path
        );

        let wasm_bytes = match self.http.get(&wasm_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                Some(resp.bytes().await?.to_vec())
            }
            _ => None,
        };

        Ok((content, wasm_bytes))
    }

    fn name(&self) -> &str {
        &self.registry_name
    }
}

// ---------------------------------------------------------------------------
// skills.sh client
// ---------------------------------------------------------------------------

/// A single skill entry from the skills.sh API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsShEntry {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub install_count: Option<u64>,
    #[serde(default)]
    pub categories: Vec<String>,
}

/// API response for skills.sh search endpoint.
#[derive(Debug, Deserialize)]
pub struct SkillsShSearchResponse {
    pub skills: Vec<SkillsShEntry>,
}

/// Client for the skills.sh aggregation API.
///
/// Provides popularity data and cross-registry search via the skills.sh
/// REST API. Uses cached results for offline browsing.
pub struct SkillsShClient {
    cache_dir: PathBuf,
    http: reqwest::Client,
}

impl SkillsShClient {
    /// Create a new skills.sh client.
    pub fn new(cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("boternity-skill-registry/0.1")
            .build()
            .unwrap_or_default();

        Self { cache_dir, http }
    }

    /// Path to the cached index file.
    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join("skills-sh-index.json")
    }

    /// Load cached index if fresh.
    fn load_cache(&self) -> Option<SkillIndex> {
        let path = self.cache_path();
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let index: SkillIndex = serde_json::from_str(&content).ok()?;

        let age = chrono::Utc::now()
            .signed_duration_since(index.last_updated)
            .num_seconds();

        if age < CACHE_FRESHNESS_SECS {
            Some(index)
        } else {
            None
        }
    }

    /// Save index to cache.
    fn save_cache(&self, index: &SkillIndex) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.cache_dir)?;
        let content = serde_json::to_string_pretty(index)?;
        std::fs::write(self.cache_path(), content)?;
        Ok(())
    }

    /// Search the skills.sh API.
    pub async fn search_api(&self, query: &str) -> anyhow::Result<Vec<SkillsShEntry>> {
        let url = format!("{}/skills/search", SKILLS_SH_BASE);

        let body = serde_json::json!({ "query": query });

        let response: SkillsShSearchResponse = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to search skills.sh")?
            .error_for_status()
            .context("skills.sh search returned error")?
            .json()
            .await
            .context("Failed to parse skills.sh search response")?;

        Ok(response.skills)
    }

    /// List skills from the skills.sh API.
    pub async fn list_api(
        &self,
        offset: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<SkillsShEntry>> {
        let url = format!(
            "{}/skills?offset={}&limit={}",
            SKILLS_SH_BASE, offset, limit
        );

        let response: SkillsShSearchResponse = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to list skills.sh")?
            .error_for_status()
            .context("skills.sh list returned error")?
            .json()
            .await
            .context("Failed to parse skills.sh list response")?;

        Ok(response.skills)
    }
}

// ---------------------------------------------------------------------------
// GitHub API types
// ---------------------------------------------------------------------------

/// Response from the GitHub Git Trees API.
#[derive(Debug, Deserialize)]
struct GitTreeResponse {
    tree: Vec<GitTreeEntry>,
}

/// A single entry in the GitHub Git tree.
#[derive(Debug, Deserialize)]
struct GitTreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_configs_returns_three_registries() {
        let configs = default_registry_configs();
        assert_eq!(configs.len(), 3);

        assert_eq!(configs[0].name, "composiohq");
        assert!(configs[0].enabled);
        assert!(matches!(
            &configs[0].registry_type,
            RegistryType::GitHub { owner, repo }
                if owner == "ComposioHQ" && repo == "awesome-claude-skills"
        ));

        assert_eq!(configs[1].name, "anthropics");
        assert!(configs[1].enabled);
        assert!(matches!(
            &configs[1].registry_type,
            RegistryType::GitHub { owner, repo }
                if owner == "anthropics" && repo == "skills"
        ));

        assert_eq!(configs[2].name, "skills-sh");
        assert!(configs[2].enabled);
        assert!(matches!(
            &configs[2].registry_type,
            RegistryType::SkillsSh
        ));
    }

    #[test]
    fn cache_path_resolution() {
        let client = GitHubRegistryClient::new(
            "ComposioHQ".to_string(),
            "awesome-claude-skills".to_string(),
            "composiohq".to_string(),
            PathBuf::from("/tmp/boternity/cache"),
        );

        let path = client.cache_path();
        assert_eq!(
            path,
            PathBuf::from("/tmp/boternity/cache/ComposioHQ-awesome-claude-skills-index.json")
        );
    }

    #[test]
    fn skills_sh_cache_path() {
        let client = SkillsShClient::new(PathBuf::from("/tmp/boternity/cache"));
        let path = client.cache_path();
        assert_eq!(
            path,
            PathBuf::from("/tmp/boternity/cache/skills-sh-index.json")
        );
    }

    #[test]
    fn skills_sh_entry_deserialization() {
        let json = r#"{
            "name": "web-search",
            "description": "Search the web",
            "repo": "ComposioHQ/awesome-claude-skills",
            "path": "skills/web-search",
            "install_count": 1234,
            "categories": ["search", "web"]
        }"#;

        let entry: SkillsShEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "web-search");
        assert_eq!(entry.description, "Search the web");
        assert_eq!(
            entry.repo.as_deref(),
            Some("ComposioHQ/awesome-claude-skills")
        );
        assert_eq!(entry.path.as_deref(), Some("skills/web-search"));
        assert_eq!(entry.install_count, Some(1234));
        assert_eq!(entry.categories, vec!["search", "web"]);
    }

    #[test]
    fn skills_sh_entry_deserialization_minimal() {
        let json = r#"{
            "name": "hello-world",
            "description": "A simple skill"
        }"#;

        let entry: SkillsShEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "hello-world");
        assert_eq!(entry.description, "A simple skill");
        assert!(entry.repo.is_none());
        assert!(entry.path.is_none());
        assert!(entry.install_count.is_none());
        assert!(entry.categories.is_empty());
    }

    #[test]
    fn skills_sh_search_response_deserialization() {
        let json = r#"{
            "skills": [
                {
                    "name": "web-search",
                    "description": "Search the web",
                    "install_count": 500
                },
                {
                    "name": "file-reader",
                    "description": "Read files",
                    "categories": ["filesystem"]
                }
            ]
        }"#;

        let response: SkillsShSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.skills.len(), 2);
        assert_eq!(response.skills[0].name, "web-search");
        assert_eq!(response.skills[1].name, "file-reader");
    }

    #[test]
    fn github_registry_client_name() {
        let client = GitHubRegistryClient::new(
            "test-owner".to_string(),
            "test-repo".to_string(),
            "test-registry".to_string(),
            PathBuf::from("/tmp/cache"),
        );

        assert_eq!(client.name(), "test-registry");
    }

    #[test]
    fn cache_load_returns_none_when_missing() {
        let tmpdir = tempfile::tempdir().unwrap();
        let client = GitHubRegistryClient::new(
            "owner".to_string(),
            "repo".to_string(),
            "test".to_string(),
            tmpdir.path().to_path_buf(),
        );

        assert!(client.load_cache().is_none());
    }

    #[test]
    fn cache_round_trip() {
        let tmpdir = tempfile::tempdir().unwrap();
        let client = GitHubRegistryClient::new(
            "owner".to_string(),
            "repo".to_string(),
            "test".to_string(),
            tmpdir.path().to_path_buf(),
        );

        let index = SkillIndex {
            skills: vec![],
            last_updated: chrono::Utc::now(),
            source: "test".to_string(),
        };

        client.save_cache(&index).unwrap();
        let loaded = client.load_cache();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.source, "test");
        assert!(loaded.skills.is_empty());
    }

    #[test]
    fn cache_expired_returns_none() {
        let tmpdir = tempfile::tempdir().unwrap();
        let client = GitHubRegistryClient::new(
            "owner".to_string(),
            "repo".to_string(),
            "test".to_string(),
            tmpdir.path().to_path_buf(),
        );

        // Create an index with a timestamp 25 hours ago
        let old_time =
            chrono::Utc::now() - chrono::Duration::hours(25);

        let index = SkillIndex {
            skills: vec![],
            last_updated: old_time,
            source: "test".to_string(),
        };

        client.save_cache(&index).unwrap();
        assert!(client.load_cache().is_none());
    }
}
