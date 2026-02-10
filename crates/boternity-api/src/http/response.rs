//! Envelope response format for all API responses.
//!
//! Every response is wrapped in a consistent envelope:
//! ```json
//! {
//!   "data": { ... },
//!   "meta": { "request_id": "...", "timestamp": "...", "response_time_ms": 5 },
//!   "errors": [],
//!   "_links": { "self": "..." }
//! }
//! ```

use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Envelope response wrapping all API data.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    /// The main response payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    /// Request metadata.
    pub meta: ApiMeta,

    /// Error list (empty on success).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ApiErrorDetail>,

    /// HATEOAS-style links for discoverability.
    #[serde(rename = "_links", skip_serializing_if = "HashMap::is_empty")]
    pub links: HashMap<String, String>,
}

/// Metadata included in every response.
#[derive(Debug, Serialize)]
pub struct ApiMeta {
    /// Unique request identifier for tracing.
    pub request_id: String,
    /// ISO-8601 timestamp of the response.
    pub timestamp: String,
    /// Response time in milliseconds.
    pub response_time_ms: u64,
}

/// Individual error detail.
#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Additional context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a success response with data.
    pub fn success(data: T, request_id: String, response_time_ms: u64) -> Self {
        Self {
            data: Some(data),
            meta: ApiMeta {
                request_id,
                timestamp: chrono::Utc::now().to_rfc3339(),
                response_time_ms,
            },
            errors: Vec::new(),
            links: HashMap::new(),
        }
    }

    /// Add a HATEOAS link.
    pub fn with_link(mut self, rel: &str, href: &str) -> Self {
        self.links.insert(rel.to_string(), href.to_string());
        self
    }
}

impl ApiResponse<()> {
    /// Create an error response (no data).
    pub fn error(
        status_code: &str,
        message: &str,
        request_id: String,
        response_time_ms: u64,
    ) -> Self {
        Self {
            data: None,
            meta: ApiMeta {
                request_id,
                timestamp: chrono::Utc::now().to_rfc3339(),
                response_time_ms,
            },
            errors: vec![ApiErrorDetail {
                code: status_code.to_string(),
                message: message.to_string(),
                details: None,
            }],
            links: HashMap::new(),
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = if self.errors.is_empty() {
            StatusCode::OK
        } else {
            // Derive status code from the error code string
            match self.errors[0].code.as_str() {
                "NOT_FOUND" | "BOT_NOT_FOUND" => StatusCode::NOT_FOUND,
                "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
                "CONFLICT" => StatusCode::CONFLICT,
                "VALIDATION_ERROR" => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        };

        let body = serde_json::to_string(&self).unwrap_or_else(|_| {
            r#"{"errors":[{"code":"SERIALIZATION_ERROR","message":"Failed to serialize response"}]}"#.to_string()
        });

        (
            status,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response()
    }
}
