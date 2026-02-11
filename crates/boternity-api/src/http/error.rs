//! Application error type mapping to HTTP status codes and envelope format.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use boternity_types::error::{BotError, SecretError, SoulError};

/// Application-level error that maps to HTTP responses.
#[derive(Debug)]
pub enum AppError {
    /// Bot-related errors.
    Bot(BotError),
    /// Soul-related errors.
    Soul(SoulError),
    /// Secret-related errors.
    Secret(SecretError),
    /// Authentication failure.
    Unauthorized(String),
    /// Validation error.
    Validation(String),
    /// Generic internal error.
    Internal(String),
}

impl From<BotError> for AppError {
    fn from(e: BotError) -> Self {
        AppError::Bot(e)
    }
}

impl From<SoulError> for AppError {
    fn from(e: SoulError) -> Self {
        AppError::Soul(e)
    }
}

impl From<SecretError> for AppError {
    fn from(e: SecretError) -> Self {
        AppError::Secret(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Bot(BotError::NotFound) => {
                (StatusCode::NOT_FOUND, "BOT_NOT_FOUND", "Bot not found".to_string())
            }
            AppError::Bot(BotError::SlugConflict(slug)) => {
                (StatusCode::CONFLICT, "SLUG_CONFLICT", format!("Slug '{slug}' already exists"))
            }
            AppError::Bot(BotError::InvalidName(msg)) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone())
            }
            AppError::Bot(BotError::InvalidStatus(msg)) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone())
            }
            AppError::Bot(BotError::SoulIntegrityViolation { expected, actual }) => {
                (StatusCode::CONFLICT, "SOUL_INTEGRITY_VIOLATION", format!("Soul integrity violation: expected hash {expected}, got {actual}"))
            }
            AppError::Bot(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "BOT_ERROR", e.to_string())
            }
            AppError::Soul(SoulError::NotFound) => {
                (StatusCode::NOT_FOUND, "SOUL_NOT_FOUND", "Soul not found".to_string())
            }
            AppError::Soul(SoulError::HashMismatch { expected, actual }) => {
                (StatusCode::CONFLICT, "INTEGRITY_ERROR", format!("Hash mismatch: expected {expected}, got {actual}"))
            }
            AppError::Soul(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "SOUL_ERROR", e.to_string())
            }
            AppError::Secret(SecretError::NotFound) => {
                (StatusCode::NOT_FOUND, "SECRET_NOT_FOUND", "Secret not found".to_string())
            }
            AppError::Secret(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "SECRET_ERROR", e.to_string())
            }
            AppError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone())
            }
            AppError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone())
            }
            AppError::Internal(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg.clone())
            }
        };

        let body = json!({
            "data": null,
            "meta": {
                "request_id": "",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "response_time_ms": 0
            },
            "errors": [{
                "code": code,
                "message": message,
            }]
        });

        (
            status,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_string(),
        )
            .into_response()
    }
}
