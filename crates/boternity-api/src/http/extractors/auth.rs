//! API key authentication extractor.
//!
//! Extracts and verifies API keys from:
//! - `Authorization: Bearer <key>` header
//! - `X-API-Key: <key>` header
//!
//! Keys are SHA-256 hashed and compared against the `api_keys` table.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::http::error::AppError;
use crate::state::AppState;

/// Authenticated request marker. Extracting this validates the API key.
pub struct Authenticated;

impl FromRequestParts<AppState> for Authenticated {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract API key from headers
        let api_key = extract_api_key(parts)?;

        // Hash the provided key
        let key_hash = hash_api_key(&api_key);

        // Verify against database
        let result = sqlx::query("SELECT id FROM api_keys WHERE key_hash = ?")
            .bind(&key_hash)
            .fetch_optional(&state.db_pool.reader)
            .await
            .map_err(|e| AppError::Internal(format!("Database error: {e}")))?;

        match result {
            Some(row) => {
                // Update last_used_at (best effort, don't fail the request)
                let id: String = row.get("id");
                let now = chrono::Utc::now().to_rfc3339();
                let _ = sqlx::query("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
                    .bind(&now)
                    .bind(&id)
                    .execute(&state.db_pool.writer)
                    .await;
                Ok(Authenticated)
            }
            None => Err(AppError::Unauthorized(
                "Invalid API key. Provide a valid key via 'Authorization: Bearer <key>' or 'X-API-Key: <key>' header.".to_string(),
            )),
        }
    }
}

/// Extract the API key from request headers.
fn extract_api_key(parts: &Parts) -> Result<String, AppError> {
    // Try Authorization: Bearer <key>
    if let Some(auth) = parts.headers.get("authorization") {
        let auth_str = auth.to_str().map_err(|_| {
            AppError::Unauthorized("Invalid Authorization header encoding".to_string())
        })?;
        if let Some(key) = auth_str.strip_prefix("Bearer ") {
            return Ok(key.trim().to_string());
        }
    }

    // Try X-API-Key header
    if let Some(key) = parts.headers.get("x-api-key") {
        let key_str = key.to_str().map_err(|_| {
            AppError::Unauthorized("Invalid X-API-Key header encoding".to_string())
        })?;
        return Ok(key_str.trim().to_string());
    }

    Err(AppError::Unauthorized(
        "Missing API key. Provide via 'Authorization: Bearer <key>' or 'X-API-Key: <key>' header.".to_string(),
    ))
}

/// Compute SHA-256 hash of an API key (lowercase hex).
pub fn hash_api_key(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    format!("{:x}", digest)
}

/// Generate a new API key and store its hash in the database.
///
/// Returns the plaintext key (shown to user once) and its hash.
pub async fn ensure_api_key(pool: &crate::state::AppState) -> anyhow::Result<String> {
    // Check if any API key exists
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM api_keys LIMIT 1")
            .fetch_optional(&pool.db_pool.reader)
            .await?;

    if existing.is_some() {
        // Key already exists, user must know it from initial creation
        return Ok("(existing key - shown only on first creation)".to_string());
    }

    // Generate a new key
    use aes_gcm::aead::{rand_core::RngCore, OsRng};
    let mut key_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut key_bytes);
    let plaintext_key = format!(
        "bnity_{}",
        key_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );

    let key_hash = hash_api_key(&plaintext_key);
    let id = uuid::Uuid::now_v7().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO api_keys (id, key_hash, name, created_at) VALUES (?, ?, 'default', ?)")
        .bind(&id)
        .bind(&key_hash)
        .bind(&now)
        .execute(&pool.db_pool.writer)
        .await?;

    Ok(plaintext_key)
}
