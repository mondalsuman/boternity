//! Bot management service.
//!
//! Orchestrates bot creation, update, deletion, and cloning. Creating a bot
//! with just a name produces a complete bot with SOUL.md, IDENTITY.md, and
//! USER.md files on disk plus database records.

use std::path::PathBuf;

use boternity_types::bot::{
    Bot, BotCategory, BotId, BotStatus, CreateBotRequest, UpdateBotRequest, slugify,
};
use boternity_types::error::BotError;
use boternity_types::soul::Soul;

use crate::repository::bot::{BotFilter, BotRepository};
use crate::repository::soul::SoulRepository;
use crate::service::fs::FileSystem;
use crate::service::hash::ContentHasher;
use crate::service::soul::{
    self, SoulService,
};

/// Service orchestrating the full bot lifecycle.
///
/// Generic over repository and infrastructure traits to maintain clean
/// architecture -- boternity-core never depends on boternity-infra.
pub struct BotService<B: BotRepository, S: SoulRepository, F: FileSystem, H: ContentHasher> {
    bot_repo: B,
    soul_service: SoulService<S, F, H>,
    data_dir: PathBuf,
}

impl<B: BotRepository, S: SoulRepository, F: FileSystem, H: ContentHasher>
    BotService<B, S, F, H>
{
    /// Create a new BotService.
    ///
    /// - `bot_repo`: persistence for bot records
    /// - `soul_service`: manages soul content, hashing, and file I/O
    /// - `data_dir`: root data directory (e.g., ~/.boternity)
    pub fn new(bot_repo: B, soul_service: SoulService<S, F, H>, data_dir: PathBuf) -> Self {
        Self {
            bot_repo,
            soul_service,
            data_dir,
        }
    }

    /// Compute the directory path for a bot: `{data_dir}/bots/{slug}/`
    fn bot_dir(&self, slug: &str) -> PathBuf {
        self.data_dir.join("bots").join(slug)
    }

    /// Create a new bot with identity files.
    ///
    /// Given just a name, this:
    /// 1. Generates a unique slug
    /// 2. Creates the bot record in the database
    /// 3. Creates SOUL.md with a human-like default personality
    /// 4. Creates IDENTITY.md with sensible LLM config defaults
    /// 5. Creates USER.md as an empty briefing template
    /// 6. Computes and stores the SHA-256 hash of SOUL.md
    pub async fn create_bot(&self, request: CreateBotRequest) -> Result<Bot, BotError> {
        // Validate name
        let name = request.name.trim().to_string();
        if name.is_empty() {
            return Err(BotError::InvalidName("name cannot be empty".to_string()));
        }

        // Generate slug and ensure uniqueness
        let base_slug = slugify(&name);
        if base_slug.is_empty() {
            return Err(BotError::InvalidName(
                "name must contain at least one alphanumeric character".to_string(),
            ));
        }

        let slug = self.ensure_unique_slug(&base_slug).await?;

        let category = request.category.unwrap_or_default();
        let now = chrono::Utc::now();

        // Create bot struct
        let bot = Bot {
            id: BotId::new(),
            slug: slug.clone(),
            name: name.clone(),
            description: request
                .description
                .unwrap_or_else(|| format!("A bot named {name}")),
            status: BotStatus::Active,
            category: category.clone(),
            tags: request.tags.unwrap_or_default(),
            user_id: None,
            conversation_count: 0,
            total_tokens_used: 0,
            version_count: 0,
            created_at: now,
            updated_at: now,
            last_active_at: None,
        };

        // Save to database
        let bot = self
            .bot_repo
            .create(&bot)
            .await
            .map_err(|e| match e {
                boternity_types::error::RepositoryError::Conflict(msg) => {
                    BotError::SlugConflict(msg)
                }
                other => BotError::StorageError(other.to_string()),
            })?;

        // Create bot directory and identity files
        let bot_dir = self.bot_dir(&slug);
        self.create_identity_files(&bot, &bot_dir, &name, &category)
            .await?;

        Ok(bot)
    }

    /// Create identity files (SOUL.md, IDENTITY.md, USER.md) for a bot.
    async fn create_identity_files(
        &self,
        bot: &Bot,
        bot_dir: &std::path::Path,
        name: &str,
        category: &BotCategory,
    ) -> Result<Soul, BotError> {
        // Generate default content
        let soul_content = soul::generate_default_soul(name);
        let identity_content = soul::generate_default_identity(name, category);
        let user_content = soul::generate_default_user(name);

        let soul_path = bot_dir.join("SOUL.md");
        let identity_path = bot_dir.join("IDENTITY.md");
        let user_path = bot_dir.join("USER.md");

        // Write SOUL.md and save version (creates directory, writes file, hashes, saves to DB)
        let soul = self
            .soul_service
            .write_and_save_soul(&bot.id, &soul_content, &soul_path)
            .await
            .map_err(|e| BotError::FileSystemError(e.to_string()))?;

        // Write IDENTITY.md
        self.soul_service
            .write_identity(&identity_content, &identity_path)
            .await
            .map_err(|e| BotError::FileSystemError(e.to_string()))?;

        // Write USER.md
        self.soul_service
            .write_user(&user_content, &user_path)
            .await
            .map_err(|e| BotError::FileSystemError(e.to_string()))?;

        Ok(soul)
    }

    /// Ensure a slug is unique by appending -2, -3, etc. if needed.
    async fn ensure_unique_slug(&self, base_slug: &str) -> Result<String, BotError> {
        let mut slug = base_slug.to_string();
        let mut counter = 2;

        loop {
            let existing = self
                .bot_repo
                .get_by_slug(&slug)
                .await
                .map_err(|e| BotError::StorageError(e.to_string()))?;

            if existing.is_none() {
                return Ok(slug);
            }

            slug = format!("{base_slug}-{counter}");
            counter += 1;

            // Safety valve: prevent infinite loops
            if counter > 100 {
                return Err(BotError::SlugConflict(format!(
                    "could not generate unique slug from '{base_slug}'"
                )));
            }
        }
    }

    /// Get a bot by ID.
    pub async fn get_bot(&self, id: &BotId) -> Result<Bot, BotError> {
        self.bot_repo
            .get_by_id(id)
            .await
            .map_err(|e| BotError::StorageError(e.to_string()))?
            .ok_or(BotError::NotFound)
    }

    /// Get a bot by slug.
    pub async fn get_bot_by_slug(&self, slug: &str) -> Result<Bot, BotError> {
        self.bot_repo
            .get_by_slug(slug)
            .await
            .map_err(|e| BotError::StorageError(e.to_string()))?
            .ok_or(BotError::NotFound)
    }

    /// List bots with optional filtering.
    pub async fn list_bots(&self, filter: Option<BotFilter>) -> Result<Vec<Bot>, BotError> {
        self.bot_repo
            .list(filter)
            .await
            .map_err(|e| BotError::StorageError(e.to_string()))
    }

    /// Update a bot's mutable fields.
    pub async fn update_bot(
        &self,
        id: &BotId,
        request: UpdateBotRequest,
    ) -> Result<Bot, BotError> {
        let mut bot = self.get_bot(id).await?;

        if let Some(name) = request.name {
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return Err(BotError::InvalidName("name cannot be empty".to_string()));
            }
            bot.name = trimmed;
        }
        if let Some(description) = request.description {
            bot.description = description;
        }
        if let Some(status) = request.status {
            bot.status = status;
        }
        if let Some(category) = request.category {
            bot.category = category;
        }
        if let Some(tags) = request.tags {
            bot.tags = tags;
        }

        bot.updated_at = chrono::Utc::now();

        self.bot_repo
            .update(&bot)
            .await
            .map_err(|e| BotError::StorageError(e.to_string()))
    }

    /// Delete a bot and remove its directory from disk.
    pub async fn delete_bot(&self, id: &BotId) -> Result<(), BotError> {
        // Get bot to find slug for directory cleanup
        let bot = self.get_bot(id).await?;
        let bot_dir = self.bot_dir(&bot.slug);

        // Delete from database (cascades to soul_versions)
        self.bot_repo
            .delete(id)
            .await
            .map_err(|e| BotError::StorageError(e.to_string()))?;

        // Remove bot directory from disk (best-effort: log but don't fail if missing)
        if self.soul_service.fs().exists(&bot_dir).await {
            self.soul_service
                .fs()
                .remove_dir_all(&bot_dir)
                .await
                .map_err(|e| BotError::FileSystemError(e.to_string()))?;
        }

        Ok(())
    }

    /// Clone a bot: copies soul + config, not history or memories.
    ///
    /// Creates a new bot with a new ID and slug derived from the original name
    /// with " (Clone)" appended.
    pub async fn clone_bot(&self, source_id: &BotId) -> Result<Bot, BotError> {
        let source = self.get_bot(source_id).await?;

        // Get current soul content
        let soul_content = self
            .soul_service
            .get_current_soul(&source.id)
            .await
            .map_err(|e| BotError::StorageError(e.to_string()))?
            .map(|s| s.content);

        let clone_name = format!("{} (Clone)", source.name);
        let request = CreateBotRequest {
            name: clone_name.clone(),
            description: Some(source.description.clone()),
            category: Some(source.category.clone()),
            tags: Some(source.tags.clone()),
        };

        // Create the clone bot (uses default soul initially)
        let clone_bot = self.create_bot(request).await?;

        // If source had a custom soul, overwrite the default
        if let Some(content) = soul_content {
            let soul_path = self.bot_dir(&clone_bot.slug).join("SOUL.md");
            // Rewrite soul with source's content
            self.soul_service
                .write_and_save_soul(&clone_bot.id, &content, &soul_path)
                .await
                .map_err(|e| BotError::StorageError(e.to_string()))?;
        }

        Ok(clone_bot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_bot_request_defaults() {
        let req = CreateBotRequest {
            name: "Luna".to_string(),
            description: None,
            category: None,
            tags: None,
        };
        assert_eq!(req.name, "Luna");
        assert!(req.description.is_none());
    }

    #[test]
    fn test_update_bot_request_defaults() {
        let req = UpdateBotRequest::default();
        assert!(req.name.is_none());
        assert!(req.description.is_none());
        assert!(req.status.is_none());
        assert!(req.category.is_none());
        assert!(req.tags.is_none());
    }
}
