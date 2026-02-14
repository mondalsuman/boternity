//! Skill registry discovery and search traits.
//!
//! Defines the `SkillRegistry` trait for discovering skills from remote sources
//! (GitHub repositories, skills.sh API, custom endpoints). Also defines the
//! types used for discovery results, registry configuration, and local caching.

use std::future::Future;

use boternity_types::skill::SkillManifest;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Discovery types
// ---------------------------------------------------------------------------

/// A skill discovered from a remote registry (not yet installed).
///
/// Contains all the metadata needed to display the skill in a browser/search
/// and to install it if the user chooses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSkill {
    /// The skill name (slug format, e.g. "web-search").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Registry source identifier (e.g. "composiohq", "skills-sh").
    pub source: String,
    /// Path within the source repository (e.g. "skills/web-search").
    pub path: String,
    /// Parsed SKILL.md manifest.
    pub manifest: SkillManifest,
    /// Number of installs/downloads (if available from registry).
    pub install_count: Option<u64>,
    /// Category tags for filtering.
    pub categories: Vec<String>,
}

// ---------------------------------------------------------------------------
// Registry configuration
// ---------------------------------------------------------------------------

/// Configuration for a single skill registry endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Display name for this registry (e.g. "composiohq").
    pub name: String,
    /// The type and connection details.
    pub registry_type: RegistryType,
    /// Whether this registry is enabled for discovery.
    pub enabled: bool,
}

/// The backing source type for a registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RegistryType {
    /// A GitHub repository containing skill directories.
    GitHub {
        /// Repository owner (e.g. "ComposioHQ").
        owner: String,
        /// Repository name (e.g. "awesome-claude-skills").
        repo: String,
    },
    /// The skills.sh aggregation API.
    SkillsSh,
    /// A custom registry endpoint (user-provided URL).
    Custom {
        /// Base URL for the custom registry API.
        url: String,
    },
}

// ---------------------------------------------------------------------------
// Cache types
// ---------------------------------------------------------------------------

/// A cached index of discovered skills from a single registry source.
///
/// Serialized to JSON and stored locally for offline browsing. The cache is
/// refreshed when older than 24 hours.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndex {
    /// All discovered skills from this source.
    pub skills: Vec<DiscoveredSkill>,
    /// When this index was last refreshed from the remote.
    pub last_updated: chrono::DateTime<chrono::Utc>,
    /// Registry source name that produced this index.
    pub source: String,
}

// ---------------------------------------------------------------------------
// SkillRegistry trait (RPITIT)
// ---------------------------------------------------------------------------

/// Trait for discovering and fetching skills from a remote registry.
///
/// Implementations handle the specific protocol (GitHub API, skills.sh REST,
/// custom endpoints) and local caching. Uses RPITIT for async methods.
pub trait SkillRegistry: Send + Sync {
    /// Search for skills matching a query string.
    ///
    /// Returns up to `limit` results ordered by relevance.
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<DiscoveredSkill>>> + Send;

    /// List skills with pagination.
    ///
    /// Returns up to `limit` results starting from `offset`.
    fn list(
        &self,
        offset: usize,
        limit: usize,
    ) -> impl Future<Output = anyhow::Result<Vec<DiscoveredSkill>>> + Send;

    /// Fetch a skill's full content (SKILL.md text and optional WASM bytes).
    ///
    /// Returns `(skill_md_content, Option<wasm_bytes>)`.
    fn fetch_skill(
        &self,
        skill: &DiscoveredSkill,
    ) -> impl Future<Output = anyhow::Result<(String, Option<Vec<u8>>)>> + Send;

    /// Human-readable name for this registry.
    fn name(&self) -> &str;
}
