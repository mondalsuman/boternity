//! Query parameter extractors for list endpoints.

use serde::Deserialize;

/// Query parameters for bot list endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct BotListQuery {
    /// Filter by status (active, disabled, archived).
    pub status: Option<String>,
    /// Filter by category (assistant, creative, research, utility).
    pub category: Option<String>,
    /// Sort by field.
    #[serde(default = "default_sort")]
    pub sort: String,
    /// Sort order (asc, desc).
    #[serde(default = "default_order")]
    pub order: String,
    /// Maximum results.
    pub limit: Option<i64>,
    /// Offset for pagination.
    pub offset: Option<i64>,
    /// Sparse fieldsets (comma-separated field names).
    #[allow(dead_code)]
    pub fields: Option<String>,
}

fn default_sort() -> String {
    "created_at".to_string()
}

fn default_order() -> String {
    "desc".to_string()
}
