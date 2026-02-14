//! Skill system domain types.
//!
//! Defines the core types for the skill system: manifests, trust tiers,
//! capabilities, permissions, audit entries, and resource limits.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core enums
// ---------------------------------------------------------------------------

/// The type of a skill: prompt-based (system prompt injection) or
/// tool-based (callable function with structured I/O).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillType {
    Prompt,
    Tool,
}

/// Trust tier determines how a skill is executed and what restrictions apply.
///
/// - `Local`: Full trust, shell access, no sandbox.
/// - `Verified`: Relaxed WASM sandbox (verified registry source).
/// - `Untrusted`: Strict WASM sandbox (unknown/untrusted source).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    Local,
    Verified,
    Untrusted,
}

impl Default for TrustTier {
    fn default() -> Self {
        Self::Untrusted
    }
}

impl fmt::Display for TrustTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Verified => write!(f, "verified"),
            Self::Untrusted => write!(f, "untrusted"),
        }
    }
}

/// Fine-grained capability that a skill may request.
///
/// Each capability maps to a specific operation the skill can perform.
/// Capabilities are approved at install time and enforced at runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadFile,
    WriteFile,
    HttpGet,
    HttpPost,
    ExecCommand,
    ReadEnv,
    RecallMemory,
    GetSecret,
}

// ---------------------------------------------------------------------------
// Manifest types (agentskills.io compatible)
// ---------------------------------------------------------------------------

/// Parsed SKILL.md YAML frontmatter, following the agentskills.io specification.
///
/// The `metadata` section contains boternity-specific extensions that keep the
/// top-level fields compatible with the open standard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub metadata: Option<SkillMetadata>,
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

/// Extended metadata for boternity skills.
///
/// Fields are placed under `metadata` in the SKILL.md frontmatter to stay
/// compatible with the agentskills.io specification while adding boternity
/// features like trust tiers, capabilities, and skill inheritance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, rename = "skill-type")]
    pub skill_type: Option<SkillType>,
    #[serde(default)]
    pub capabilities: Option<Vec<Capability>>,
    #[serde(default)]
    pub dependencies: Option<Vec<String>>,
    #[serde(default, rename = "conflicts-with")]
    pub conflicts_with: Option<Vec<String>>,
    #[serde(default, rename = "trust-tier")]
    pub trust_tier: Option<TrustTier>,
    #[serde(default)]
    pub parents: Option<Vec<String>>,
    #[serde(default)]
    pub secrets: Option<Vec<String>>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Permission types
// ---------------------------------------------------------------------------

/// A record of a single capability grant or denial for a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionGrant {
    pub skill_name: String,
    pub capability: Capability,
    pub granted: bool,
    pub granted_at: DateTime<Utc>,
}

/// The complete permission state for a skill on a specific bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPermissions {
    pub skill_name: String,
    pub trust_tier: TrustTier,
    pub grants: Vec<PermissionGrant>,
    /// Whether the user has manually escalated the trust tier.
    pub escalated: bool,
}

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Per-bot configuration for a single skill (from skills.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSkillConfig {
    pub skill_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub trust_tier: Option<TrustTier>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
    #[serde(default)]
    pub capabilities: Option<Vec<Capability>>,
}

fn default_true() -> bool {
    true
}

/// The per-bot `skills.toml` file, mapping skill names to their configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSkillsFile {
    pub skills: HashMap<String, BotSkillConfig>,
}

// ---------------------------------------------------------------------------
// Audit types
// ---------------------------------------------------------------------------

/// Audit log entry for every skill invocation.
///
/// Captures all observable data about a skill execution for security review
/// and debugging. Input/output are stored as SHA-256 hashes for privacy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAuditEntry {
    pub invocation_id: Uuid,
    pub skill_name: String,
    pub skill_version: String,
    pub trust_tier: TrustTier,
    pub capabilities_used: Vec<Capability>,
    /// SHA-256 hash of the input (not raw input, for privacy).
    pub input_hash: String,
    /// SHA-256 hash of the output.
    pub output_hash: String,
    pub fuel_consumed: Option<u64>,
    pub memory_peak_bytes: Option<usize>,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub bot_id: Uuid,
}

// ---------------------------------------------------------------------------
// Installed skill types
// ---------------------------------------------------------------------------

/// A fully loaded skill with manifest, body, and source metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    pub manifest: SkillManifest,
    /// The markdown body from SKILL.md (instructions below frontmatter).
    pub body: String,
    pub source: SkillSource,
    pub install_path: PathBuf,
    /// Path to compiled WASM component (only for tool-based skills).
    pub wasm_path: Option<PathBuf>,
}

/// Where a skill was sourced from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillSource {
    Local,
    Registry {
        registry_name: String,
        repo: String,
        path: String,
    },
}

/// Metadata stored in `.boternity-meta.toml` alongside installed registry skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub source: SkillSource,
    pub installed_at: DateTime<Utc>,
    pub version: semver::Version,
    pub checksum: String,
    pub trust_tier: TrustTier,
}

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

/// Configurable resource limits for WASM skill execution.
///
/// Defaults are tuned for a reasonable balance between allowing useful work
/// and preventing runaway resource consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum linear memory in bytes (default: 64 MB).
    #[serde(default = "default_max_memory_bytes")]
    pub max_memory_bytes: usize,
    /// Maximum fuel units for CPU limiting (default: 1,000,000).
    #[serde(default = "default_max_fuel")]
    pub max_fuel: u64,
    /// Maximum execution duration in milliseconds (default: 30,000).
    #[serde(default = "default_max_duration_ms")]
    pub max_duration_ms: u64,
}

fn default_max_memory_bytes() -> usize {
    64 * 1024 * 1024 // 64 MB
}

fn default_max_fuel() -> u64 {
    1_000_000
}

fn default_max_duration_ms() -> u64 {
    30_000
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: default_max_memory_bytes(),
            max_fuel: default_max_fuel(),
            max_duration_ms: default_max_duration_ms(),
        }
    }
}
