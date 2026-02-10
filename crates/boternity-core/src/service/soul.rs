//! Soul management service.
//!
//! Handles SOUL.md generation, hashing, and integrity verification.
//! Generates default human-like personalities -- never generic bot/assistant personas.

use std::path::Path;

use boternity_types::bot::{BotCategory, BotId};
use boternity_types::error::SoulError;
use boternity_types::identity::Identity;
use boternity_types::soul::{Soul, SoulId, SoulVersion};

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

        // Create soul version
        let soul = Soul {
            id: SoulId::new(),
            bot_id: bot_id.clone(),
            content: content.to_string(),
            hash,
            version: 1,
            created_at: chrono::Utc::now(),
        };

        // Save to repository
        self.soul_repo
            .save_version(&soul)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?;

        Ok(soul)
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

    /// Verify soul integrity by comparing the file on disk with the stored hash.
    ///
    /// Returns Ok(true) if hashes match, Ok(false) if mismatch, Err if file or hash missing.
    pub async fn verify_soul_integrity(
        &self,
        bot_id: &BotId,
        soul_path: &Path,
    ) -> Result<bool, SoulError> {
        // Read the file from disk
        let file_content = self
            .fs
            .read_file(soul_path)
            .await
            .map_err(|e| SoulError::FileSystemError(e.to_string()))?;

        // Compute hash of file content
        let file_hash = self.hasher.compute_hash(&file_content);

        // Get stored hash from database
        let stored_hash = self
            .soul_repo
            .get_stored_hash(bot_id)
            .await
            .map_err(|e| SoulError::StorageError(e.to_string()))?
            .ok_or(SoulError::NotFound)?;

        Ok(file_hash == stored_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::bot::BotCategory;

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
}
