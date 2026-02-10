//! HTTP/REST API layer for Boternity.
//!
//! Axum-based REST API at `/api/v1/` with API key authentication,
//! envelope response format, and CORS support.

pub mod error;
pub mod extractors;
pub mod handlers;
pub mod response;
pub mod router;
