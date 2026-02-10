//! Axum router configuration with middleware.
//!
//! All routes are under `/api/v1/`.
//! Middleware: CORS, tracing, response time.

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::http::handlers;
use crate::state::AppState;

/// Build the complete API router with all routes and middleware.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        // Bot CRUD
        .route("/bots", post(handlers::bot::create_bot))
        .route("/bots", get(handlers::bot::list_bots))
        .route("/bots/{id}", get(handlers::bot::get_bot))
        .route("/bots/{id}", put(handlers::bot::update_bot))
        .route("/bots/{id}", delete(handlers::bot::delete_bot))
        .route("/bots/{id}/clone", post(handlers::bot::clone_bot))
        // Soul
        .route("/bots/{id}/soul", get(handlers::soul::get_soul))
        .route(
            "/bots/{id}/soul/versions",
            get(handlers::soul::get_soul_versions),
        )
        // Secrets
        .route("/secrets", get(handlers::secret::list_secrets))
        .route("/secrets/{key}", put(handlers::secret::set_secret))
        .route("/secrets/{key}", delete(handlers::secret::delete_secret));

    Router::new()
        .nest("/api/v1", api_routes)
        .route("/health", get(health_check))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// GET /health - Simple health check endpoint (no auth required).
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
