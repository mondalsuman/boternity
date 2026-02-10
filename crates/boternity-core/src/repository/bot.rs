//! Bot repository trait definition.

use boternity_types::bot::{Bot, BotCategory, BotId, BotStatus};
use boternity_types::error::RepositoryError;

use super::SortOrder;

/// Filter criteria for listing bots.
#[derive(Debug, Clone, Default)]
pub struct BotFilter {
    /// Filter by lifecycle status.
    pub status: Option<BotStatus>,
    /// Filter by category.
    pub category: Option<BotCategory>,
    /// Field to sort by (e.g., "created_at", "name", "last_active_at").
    pub sort_by: Option<String>,
    /// Sort direction.
    pub sort_order: Option<SortOrder>,
    /// Maximum number of results.
    pub limit: Option<i64>,
    /// Number of results to skip (offset pagination).
    pub offset: Option<i64>,
}

/// Repository trait for bot persistence.
///
/// Implementations live in boternity-infra (e.g., SqliteBotRepository).
/// Uses native async fn in traits (Rust 2024 edition, no async_trait macro).
pub trait BotRepository: Send + Sync {
    /// Create a new bot. Returns the created bot.
    fn create(
        &self,
        bot: &Bot,
    ) -> impl std::future::Future<Output = Result<Bot, RepositoryError>> + Send;

    /// Get a bot by its unique ID.
    fn get_by_id(
        &self,
        id: &BotId,
    ) -> impl std::future::Future<Output = Result<Option<Bot>, RepositoryError>> + Send;

    /// Get a bot by its unique slug.
    fn get_by_slug(
        &self,
        slug: &str,
    ) -> impl std::future::Future<Output = Result<Option<Bot>, RepositoryError>> + Send;

    /// List bots with optional filtering, sorting, and pagination.
    fn list(
        &self,
        filter: Option<BotFilter>,
    ) -> impl std::future::Future<Output = Result<Vec<Bot>, RepositoryError>> + Send;

    /// Update an existing bot. Returns the updated bot.
    fn update(
        &self,
        bot: &Bot,
    ) -> impl std::future::Future<Output = Result<Bot, RepositoryError>> + Send;

    /// Permanently delete a bot by ID.
    fn delete(
        &self,
        id: &BotId,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;
}
