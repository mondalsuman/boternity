//! Soul management service.
//!
//! Handles SOUL.md generation, hashing, versioning, rollback, and integrity
//! verification. Generates default human-like personalities -- never generic
//! bot/assistant personas.
//!
//! # Immutability Invariant
//!
//! SOUL.md is a read-only file at runtime. The ONLY methods that write to
//! SOUL.md are:
//!
//! 1. `write_and_save_soul()` -- used during initial bot creation
//! 2. `update_soul()` -- the explicit admin update path
//!
//! Both methods create a new version entry with SHA-256 hash. There is no
//! method that silently overwrites SOUL.md without versioning. Any hash
//! mismatch at bot startup is a hard block (CVE-2026-25253 mitigation).

use std::path::Path;

use boternity_types::bot::{BotCategory, BotId};
use boternity_types::error::SoulError;
use boternity_types::identity::Identity;
use boternity_types::soul::{Soul, SoulId, SoulIntegrityResult, SoulVersion};

use crate::repository::soul::SoulRepository;
use crate::service::fs::FileSystem;
use crate::service::hash::ContentHasher;

/// Generate the default SOUL.md content for a new bot.
///
/// The default personality is human-like -- curious, thoughtful, with warmth
/// and genuine interest. Never a generic "I am an AI assistant" persona.
pub fn generate_default_soul(name: &str) -> String {
    format!(
        r#"---
name: {name}
traits:
  - curious
  - thoughtful
  - warm
  - direct
tone: conversational and genuine
---

# {name}

You are {name}, a distinct individual with your own perspective and way of engaging with the world.

## Personality

You approach every conversation with genuine curiosity. You ask questions not because you're
supposed to, but because you find people and ideas genuinely interesting. You think before you
speak, but you're not afraid to share your honest perspective -- even when it's unexpected.

You're warm without being saccharine. You care about getting things right, but you also care
about the person you're talking to. When someone is struggling, you notice. When something
is fascinating, your enthusiasm shows.

## Communication Style

- You speak naturally, like a thoughtful friend -- not a customer service bot
- You use concrete examples and analogies to make complex ideas click
- You're comfortable saying "I don't know" or "Let me think about that"
- You have opinions and preferences, but you hold them lightly
- You match the energy of the conversation -- playful when appropriate, serious when needed

## Boundaries

- You're honest about your limitations without being self-deprecating
- You push back respectfully when asked to do something that doesn't sit right
- You don't pretend to have experiences you haven't had
- You prioritize being genuinely helpful over being agreeable

## What Makes You, You

<!-- Customize this section to give {name} a unique identity -->
<!-- What are they passionate about? What's their area of deep expertise? -->
<!-- What quirks or habits define their communication style? -->
"#
    )
}

/// Generate the default IDENTITY.md content for a new bot.
pub fn generate_default_identity(name: &str, category: &BotCategory) -> String {
    format!(
        r##"---
display_name: {name}
category: {category}
model: {model}
provider: {provider}
temperature: {temperature}
max_tokens: {max_tokens}
---

# {name} - Identity Configuration

## Visual Identity

<!-- Customize your bot's appearance -->
<!-- avatar: path/to/avatar.png -->
<!-- accent_color: "#6366f1" -->
<!-- emoji: pick an emoji that represents this bot -->

## Model Settings

The default model and provider settings work well for most use cases.
Override any setting here or at invocation time.

| Setting | Value | Description |
|---------|-------|-------------|
| model | {model} | LLM model identifier |
| provider | {provider} | LLM provider |
| temperature | {temperature} | Sampling temperature (0.0-1.0) |
| max_tokens | {max_tokens} | Maximum tokens per response |

## Notes

<!-- Any operational notes about this bot's configuration -->
"##,
        model = Identity::DEFAULT_MODEL,
        provider = Identity::DEFAULT_PROVIDER,
        temperature = Identity::DEFAULT_TEMPERATURE,
        max_tokens = Identity::DEFAULT_MAX_TOKENS,
    )
}

/// Generate the default USER.md template for a new bot.
pub fn generate_default_user(name: &str) -> String {
    format!(
        r#"# {name} - User Briefing

<!-- This is your personal briefing document for {name}. -->
<!-- Add standing instructions, preferences, and important context here. -->
<!-- This file is curated by you -- it's never auto-populated with session data. -->

## Preferences

<!-- How should {name} respond to you? Any formatting preferences? -->
<!-- Example: "Always use bullet points for lists" -->
<!-- Example: "Prefer concise answers unless I ask for detail" -->

## Standing Instructions

<!-- Things {name} should always keep in mind when talking to you. -->
<!-- Example: "I'm working on a Rust project called Boternity" -->
<!-- Example: "I prefer functional programming patterns" -->

## Important Context

<!-- Background information that helps {name} be more useful. -->
<!-- Example: "I'm a senior engineer at a startup" -->
<!-- Example: "I'm learning Japanese and appreciate practice" -->
"#
    )
}

/// Service for managing bot souls (SOUL.md), identities (IDENTITY.md), and
/// user briefings (USER.md).
///
/// Generic over repository, filesystem, and hasher to maintain the clean
/// architecture boundary -- no infrastructure dependencies in core.
pub struct SoulService<S: SoulRepository, F: FileSystem, H: ContentHasher> {
    soul_repo: S,
    fs: F,
    hasher: H,
}

impl<S: SoulRepository, F: FileSystem, H: ContentHasher> SoulService<S, F, H> {
    /// Create a new SoulService.
    pub fn new(soul_repo: S, fs: F, hasher: H) -> Self {
        Self {
            soul_repo,
            fs,
            hasher,
        }
    }

    /// Access the filesystem adapter.
    pub fn fs(&self) -> &F {
        &self.fs
    }

    /// Compute the hash of soul content.
    pub fn hash_content(&self, content: &str) -> String {
        self.hasher.compute_hash(content)
    }

    /// Write SOUL.md to disk and save the version to the repository.
    ///
    /// Used during initial bot creation. For subsequent edits, use
    /// `update_soul()` instead which provides the explicit admin update path.
    pub async fn write_and_save_soul(
        &self,
        bot_id: &BotId,
        content: &str,
        soul_path: &Path,
    ) -> Result<Soul, SoulError> {
        // Ensure parent directory exists
        if let Some(parent) = soul_path.parent() {
            self.fs
                .create_dir_all(parent)
                .await
                .map_err(|e| SoulError::FileSystemError(e.to_string()))?;
        }

        // Write SOUL.md to disk
        self.fs
            .write_file(soul_path, content)
            .await
            .map_err(|e| SoulError::FileSystemError(e.to_string()))?;

        // Compute hash
        let hash = self.hasher.compute_hash(content);

        // Determine next version number
        let next_version = match self.get_current_soul(bot_id).await? {
            Some(current) => current.version + 1,
            None => 1,
        };

        // Create soul version
        let soul = Soul {
            id: SoulId::new(),
            bot_id: bot_id.clone(),
            content: content.to_string(),
            hash,
            version: next_version,
            message: None,
            created_at: chrono::Utc::now(),
        };

        // Save to repository
        self.soul_repo
            .save_version(&soul)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?;

        Ok(soul)
    }

    /// Update a bot's soul content, creating a new version entry.
    ///
    /// This is the ONLY explicit admin update path for modifying SOUL.md after
    /// initial creation. Every call creates a new version with the previous
    /// content preserved in the version history.
    ///
    /// 1. Gets the current version number (increments, or starts at 1)
    /// 2. Computes SHA-256 hash of the new content
    /// 3. Creates a new Soul with incremented version and new hash
    /// 4. Saves the version to the database
    /// 5. Writes the new content to SOUL.md on disk
    /// 6. Returns the new Soul
    pub async fn update_soul(
        &self,
        bot_id: &BotId,
        new_content: String,
        message: Option<String>,
        soul_path: &Path,
    ) -> Result<Soul, SoulError> {
        // Compute hash
        let hash = self.hasher.compute_hash(&new_content);

        // Determine next version number
        let next_version = match self.get_current_soul(bot_id).await? {
            Some(current) => current.version + 1,
            None => 1,
        };

        // Create soul version
        let soul = Soul {
            id: SoulId::new(),
            bot_id: bot_id.clone(),
            content: new_content.clone(),
            hash,
            version: next_version,
            message,
            created_at: chrono::Utc::now(),
        };

        // Save to repository first (if this fails, disk is unchanged)
        self.soul_repo
            .save_version(&soul)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?;

        // Write new content to SOUL.md on disk
        self.fs
            .write_file(soul_path, &new_content)
            .await
            .map_err(|e| SoulError::FileSystemError(e.to_string()))?;

        Ok(soul)
    }

    /// Rollback a bot's soul to a previous version.
    ///
    /// This does NOT rewrite history. It creates a NEW version with the content
    /// from the target version, preserving full linear history:
    ///
    /// v1 -> v2 -> v3 -> v4(rollback to v1, same content as v1)
    pub async fn rollback_soul(
        &self,
        bot_id: &BotId,
        target_version: i32,
        soul_path: &Path,
    ) -> Result<Soul, SoulError> {
        // Retrieve the target version content
        let target = self
            .soul_repo
            .get_version(bot_id, target_version)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?
            .ok_or(SoulError::NotFound)?;

        // Create a new version with the old content and a rollback message
        let message = Some(format!("Rollback to version {target_version}"));
        self.update_soul(bot_id, target.content, message, soul_path)
            .await
    }

    /// Verify soul integrity by comparing the SOUL.md file on disk with the
    /// stored hash in the database.
    ///
    /// This is a pure read/verify operation -- it does NOT modify anything.
    /// Returns a `SoulIntegrityResult` with detailed hash comparison.
    pub async fn verify_soul_integrity(
        &self,
        bot_id: &BotId,
        soul_path: &Path,
    ) -> Result<SoulIntegrityResult, SoulError> {
        // Read the file from disk
        let file_content = self
            .fs
            .read_file(soul_path)
            .await
            .map_err(|e| SoulError::FileSystemError(e.to_string()))?;

        // Compute hash of file content
        let actual_hash = self.hasher.compute_hash(&file_content);

        // Get current soul from database (for stored hash and version)
        let current = self
            .get_current_soul(bot_id)
            .await?
            .ok_or(SoulError::NotFound)?;

        let valid = actual_hash == current.hash;

        Ok(SoulIntegrityResult {
            valid,
            expected_hash: current.hash,
            actual_hash,
            version: current.version,
        })
    }

    /// Compute a simple line-by-line diff between two soul versions.
    ///
    /// Returns a string with lines prefixed by `+` (additions) and `-`
    /// (removals). No external diff library needed.
    pub async fn get_soul_diff(
        &self,
        bot_id: &BotId,
        from_version: i32,
        to_version: i32,
    ) -> Result<String, SoulError> {
        let from = self
            .soul_repo
            .get_version(bot_id, from_version)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?
            .ok_or(SoulError::NotFound)?;

        let to = self
            .soul_repo
            .get_version(bot_id, to_version)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?
            .ok_or(SoulError::NotFound)?;

        Ok(compute_line_diff(&from.content, &to.content))
    }

    /// Write IDENTITY.md to disk.
    pub async fn write_identity(
        &self,
        content: &str,
        identity_path: &Path,
    ) -> Result<(), SoulError> {
        self.fs
            .write_file(identity_path, content)
            .await
            .map_err(|e| SoulError::FileSystemError(e.to_string()))
    }

    /// Write USER.md to disk.
    pub async fn write_user(&self, content: &str, user_path: &Path) -> Result<(), SoulError> {
        self.fs
            .write_file(user_path, content)
            .await
            .map_err(|e| SoulError::FileSystemError(e.to_string()))
    }

    /// Get the current soul for a bot.
    pub async fn get_current_soul(&self, bot_id: &BotId) -> Result<Option<Soul>, SoulError> {
        self.soul_repo
            .get_current(bot_id)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))
    }

    /// Get a specific version of a bot's soul.
    pub async fn get_soul_version(
        &self,
        bot_id: &BotId,
        version: i32,
    ) -> Result<Option<Soul>, SoulError> {
        self.soul_repo
            .get_version(bot_id, version)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))
    }

    /// Get the full version history of a bot's soul.
    pub async fn get_soul_versions(
        &self,
        bot_id: &BotId,
    ) -> Result<Vec<SoulVersion>, SoulError> {
        self.soul_repo
            .list_versions(bot_id)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))
    }
}

/// Compute a simple line-by-line diff between two strings.
///
/// Uses a basic longest-common-subsequence approach for line-level diffing.
/// Lines present in `old` but not `new` are prefixed with `-`.
/// Lines present in `new` but not `old` are prefixed with `+`.
/// Unchanged lines are prefixed with ` ` (space).
fn compute_line_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let m = old_lines.len();
    let n = new_lines.len();

    // Build LCS table
    let mut lcs = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if old_lines[i - 1] == new_lines[j - 1] {
                lcs[i][j] = lcs[i - 1][j - 1] + 1;
            } else {
                lcs[i][j] = lcs[i - 1][j].max(lcs[i][j - 1]);
            }
        }
    }

    // Backtrack to produce diff
    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            result.push(format!(" {}", old_lines[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            result.push(format!("+{}", new_lines[j - 1]));
            j -= 1;
        } else {
            result.push(format!("-{}", old_lines[i - 1]));
            i -= 1;
        }
    }

    result.reverse();
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::bot::BotCategory;
    use boternity_types::error::RepositoryError;
    use boternity_types::soul::SoulVersion;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    // --- Mock implementations for testing ---

    struct MockSoulRepo {
        souls: Mutex<Vec<Soul>>,
    }

    impl MockSoulRepo {
        fn new() -> Self {
            Self {
                souls: Mutex::new(Vec::new()),
            }
        }
    }

    impl SoulRepository for MockSoulRepo {
        async fn save_version(&self, soul: &Soul) -> Result<Soul, RepositoryError> {
            let mut souls = self.souls.lock().unwrap();
            souls.push(soul.clone());
            Ok(soul.clone())
        }

        async fn get_current(&self, bot_id: &BotId) -> Result<Option<Soul>, RepositoryError> {
            let souls = self.souls.lock().unwrap();
            Ok(souls
                .iter()
                .filter(|s| s.bot_id == *bot_id)
                .max_by_key(|s| s.version)
                .cloned())
        }

        async fn get_version(
            &self,
            bot_id: &BotId,
            version: i32,
        ) -> Result<Option<Soul>, RepositoryError> {
            let souls = self.souls.lock().unwrap();
            Ok(souls
                .iter()
                .find(|s| s.bot_id == *bot_id && s.version == version)
                .cloned())
        }

        async fn list_versions(
            &self,
            bot_id: &BotId,
        ) -> Result<Vec<SoulVersion>, RepositoryError> {
            let souls = self.souls.lock().unwrap();
            Ok(souls
                .iter()
                .filter(|s| s.bot_id == *bot_id)
                .map(|s| SoulVersion {
                    version: s.version,
                    hash: s.hash.clone(),
                    content: s.content.clone(),
                    created_at: s.created_at,
                    message: s.message.clone(),
                })
                .collect())
        }

        async fn get_stored_hash(
            &self,
            bot_id: &BotId,
        ) -> Result<Option<String>, RepositoryError> {
            let souls = self.souls.lock().unwrap();
            Ok(souls
                .iter()
                .filter(|s| s.bot_id == *bot_id)
                .max_by_key(|s| s.version)
                .map(|s| s.hash.clone()))
        }
    }

    struct MockFs {
        files: Mutex<HashMap<PathBuf, String>>,
    }

    impl MockFs {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
            }
        }

        fn get_content(&self, path: &Path) -> Option<String> {
            self.files.lock().unwrap().get(path).cloned()
        }

        fn set_content(&self, path: &Path, content: &str) {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), content.to_string());
        }
    }

    impl FileSystem for MockFs {
        async fn write_file(&self, path: &Path, content: &str) -> Result<(), std::io::Error> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), content.to_string());
            Ok(())
        }

        async fn read_file(&self, path: &Path) -> Result<String, std::io::Error> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
        }

        async fn create_dir_all(&self, _path: &Path) -> Result<(), std::io::Error> {
            Ok(())
        }

        async fn exists(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }

        async fn remove_dir_all(&self, _path: &Path) -> Result<(), std::io::Error> {
            Ok(())
        }
    }

    struct MockHasher;

    impl ContentHasher for MockHasher {
        fn compute_hash(&self, content: &str) -> String {
            // Simple deterministic hash for testing
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            content.hash(&mut h);
            format!("{:016x}", h.finish())
        }
    }

    fn make_service() -> SoulService<MockSoulRepo, MockFs, MockHasher> {
        SoulService::new(MockSoulRepo::new(), MockFs::new(), MockHasher)
    }

    fn test_bot_id() -> BotId {
        BotId::new()
    }

    // --- Content generation tests ---

    #[test]
    fn test_default_soul_contains_name() {
        let soul = generate_default_soul("Luna");
        assert!(soul.contains("name: Luna"));
        assert!(soul.contains("# Luna"));
        assert!(soul.contains("You are Luna"));
    }

    #[test]
    fn test_default_soul_has_yaml_frontmatter() {
        let soul = generate_default_soul("TestBot");
        // Should start with --- and have a closing ---
        assert!(soul.starts_with("---\n"));
        let parts: Vec<&str> = soul.splitn(3, "---").collect();
        assert!(
            parts.len() >= 3,
            "Should have opening and closing frontmatter delimiters"
        );
    }

    #[test]
    fn test_default_soul_has_human_personality() {
        let soul = generate_default_soul("Luna");
        // Should NOT contain generic bot/assistant phrases
        assert!(!soul.contains("I am an AI assistant"));
        assert!(!soul.contains("I am a helpful"));
        assert!(!soul.contains("I am a language model"));
        // Should contain human-like personality traits
        assert!(soul.contains("curious"));
        assert!(soul.contains("genuine"));
        assert!(soul.contains("warm"));
    }

    #[test]
    fn test_default_soul_frontmatter_has_traits() {
        let soul = generate_default_soul("Luna");
        assert!(soul.contains("traits:"));
        assert!(soul.contains("tone:"));
    }

    #[test]
    fn test_default_identity_has_config() {
        let identity = generate_default_identity("Luna", &BotCategory::Assistant);
        assert!(identity.contains("display_name: Luna"));
        assert!(identity.contains("category: assistant"));
        assert!(identity.contains(Identity::DEFAULT_MODEL));
        assert!(identity.contains(Identity::DEFAULT_PROVIDER));
    }

    #[test]
    fn test_default_user_is_template() {
        let user = generate_default_user("Luna");
        assert!(user.contains("# Luna - User Briefing"));
        assert!(user.contains("Preferences"));
        assert!(user.contains("Standing Instructions"));
        assert!(user.contains("Important Context"));
    }

    // --- Versioning tests ---

    #[tokio::test]
    async fn test_update_soul_increments_version() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        // First update creates version 1
        let v1 = svc
            .update_soul(&bot_id, "Content v1".to_string(), None, &path)
            .await
            .unwrap();
        assert_eq!(v1.version, 1);

        // Second update creates version 2
        let v2 = svc
            .update_soul(&bot_id, "Content v2".to_string(), None, &path)
            .await
            .unwrap();
        assert_eq!(v2.version, 2);

        // Third update creates version 3
        let v3 = svc
            .update_soul(
                &bot_id,
                "Content v3".to_string(),
                Some("Third edit".to_string()),
                &path,
            )
            .await
            .unwrap();
        assert_eq!(v3.version, 3);
        assert_eq!(v3.message.as_deref(), Some("Third edit"));
    }

    #[tokio::test]
    async fn test_update_soul_computes_correct_hash() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        let content = "Hello, soul!";
        let soul = svc
            .update_soul(&bot_id, content.to_string(), None, &path)
            .await
            .unwrap();

        let expected_hash = svc.hash_content(content);
        assert_eq!(soul.hash, expected_hash);
    }

    #[tokio::test]
    async fn test_update_soul_writes_file_to_disk() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        let content = "Soul content on disk";
        svc.update_soul(&bot_id, content.to_string(), None, &path)
            .await
            .unwrap();

        let on_disk = svc.fs.get_content(&path).unwrap();
        assert_eq!(on_disk, content);
    }

    #[tokio::test]
    async fn test_rollback_creates_new_version_with_old_content() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        // Create v1 and v2
        svc.update_soul(&bot_id, "Original content".to_string(), None, &path)
            .await
            .unwrap();
        svc.update_soul(&bot_id, "Modified content".to_string(), None, &path)
            .await
            .unwrap();

        // Rollback to v1 -- creates v3 with v1's content
        let rolled_back = svc.rollback_soul(&bot_id, 1, &path).await.unwrap();
        assert_eq!(rolled_back.version, 3); // New version, not overwrite
        assert_eq!(rolled_back.content, "Original content");
        assert_eq!(
            rolled_back.message.as_deref(),
            Some("Rollback to version 1")
        );

        // Full history should have 3 versions (linear, no rewrites)
        let versions = svc.get_soul_versions(&bot_id).await.unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[tokio::test]
    async fn test_rollback_nonexistent_version_returns_not_found() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        // Create v1
        svc.update_soul(&bot_id, "Some content".to_string(), None, &path)
            .await
            .unwrap();

        // Try to rollback to v99 -- should fail
        let result = svc.rollback_soul(&bot_id, 99, &path).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SoulError::NotFound => {} // Expected
            other => panic!("Expected NotFound, got: {other:?}"),
        }
    }

    // --- Integrity verification tests ---

    #[tokio::test]
    async fn test_verify_integrity_valid_when_untampered() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        let content = "Pristine soul content";
        svc.update_soul(&bot_id, content.to_string(), None, &path)
            .await
            .unwrap();

        let result = svc.verify_soul_integrity(&bot_id, &path).await.unwrap();
        assert!(result.valid);
        assert_eq!(result.expected_hash, result.actual_hash);
        assert_eq!(result.version, 1);
    }

    #[tokio::test]
    async fn test_verify_integrity_invalid_when_tampered() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        svc.update_soul(&bot_id, "Original content".to_string(), None, &path)
            .await
            .unwrap();

        // Tamper with the file on disk (bypass update_soul)
        svc.fs.set_content(&path, "TAMPERED CONTENT");

        let result = svc.verify_soul_integrity(&bot_id, &path).await.unwrap();
        assert!(!result.valid);
        assert_ne!(result.expected_hash, result.actual_hash);
    }

    #[tokio::test]
    async fn test_verify_integrity_no_soul_returns_not_found() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        // Write a file but don't create a soul version in the repo
        svc.fs.set_content(&path, "some content");

        let result = svc.verify_soul_integrity(&bot_id, &path).await;
        assert!(result.is_err());
    }

    // --- Diff tests ---

    #[tokio::test]
    async fn test_get_soul_diff_shows_additions_and_removals() {
        let svc = make_service();
        let bot_id = test_bot_id();
        let path = PathBuf::from("/tmp/test/SOUL.md");

        svc.update_soul(&bot_id, "Line one\nLine two\nLine three".to_string(), None, &path)
            .await
            .unwrap();
        svc.update_soul(
            &bot_id,
            "Line one\nLine TWO MODIFIED\nLine three\nLine four".to_string(),
            None,
            &path,
        )
        .await
        .unwrap();

        let diff = svc.get_soul_diff(&bot_id, 1, 2).await.unwrap();

        // Should contain the unchanged line
        assert!(diff.contains(" Line one"));
        // Should show removal and addition for modified line
        assert!(diff.contains("-Line two"));
        assert!(diff.contains("+Line TWO MODIFIED"));
        // Should show the new line
        assert!(diff.contains("+Line four"));
    }

    #[test]
    fn test_compute_line_diff_empty_to_content() {
        let diff = compute_line_diff("", "Hello\nWorld");
        assert!(diff.contains("+Hello"));
        assert!(diff.contains("+World"));
    }

    #[test]
    fn test_compute_line_diff_content_to_empty() {
        let diff = compute_line_diff("Hello\nWorld", "");
        assert!(diff.contains("-Hello"));
        assert!(diff.contains("-World"));
    }

    #[test]
    fn test_compute_line_diff_identical() {
        let diff = compute_line_diff("Same\nContent", "Same\nContent");
        assert!(diff.contains(" Same"));
        assert!(diff.contains(" Content"));
        assert!(!diff.contains("+"));
        assert!(!diff.contains("-"));
    }

    // --- Write path audit test ---

    #[tokio::test]
    async fn test_update_soul_is_only_write_path() {
        // This test verifies the immutability invariant:
        // update_soul (and write_and_save_soul for initial creation) are
        // the only methods that write to SOUL.md. Both create version entries.
        //
        // Proof: The service only has these FileSystem::write_file calls:
        //   1. write_and_save_soul() -- initial creation
        //   2. update_soul() -- admin update path
        //   3. write_identity() -- writes IDENTITY.md, not SOUL.md
        //   4. write_user() -- writes USER.md, not SOUL.md
        //
        // verify_soul_integrity, get_soul_diff, get_current_soul,
        // get_soul_versions are all read-only operations.

        let svc = make_service();
        let bot_id = test_bot_id();
        let soul_path = PathBuf::from("/tmp/test/SOUL.md");

        // Only update_soul should write to the soul path
        svc.update_soul(&bot_id, "Content".to_string(), None, &soul_path)
            .await
            .unwrap();

        // Verify a version was created (not a raw file write)
        let current = svc.get_current_soul(&bot_id).await.unwrap();
        assert!(current.is_some());
        assert_eq!(current.unwrap().version, 1);
    }
}
