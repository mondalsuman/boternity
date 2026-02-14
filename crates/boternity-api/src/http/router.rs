//! Axum router configuration with middleware.
//!
//! All REST routes are under `/api/v1/`.
//! WebSocket endpoint at `/ws/events` (outside the REST namespace).
//! Middleware: CORS, tracing, response time.
//!
//! In production, the built React SPA is served from `apps/web/dist/`
//! (configurable via `BOTERNITY_WEB_DIR`). API routes take priority;
//! unknown paths fall through to the SPA's `index.html` for client-side
//! routing. If the directory does not exist, only the API is served.

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
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
        .route(
            "/bots/{id}/soul",
            get(handlers::soul::get_soul).put(handlers::soul::update_soul),
        )
        .route(
            "/bots/{id}/soul/versions",
            get(handlers::soul::get_soul_versions),
        )
        .route(
            "/bots/{id}/soul/versions/{version}",
            get(handlers::soul::get_soul_version),
        )
        .route(
            "/bots/{id}/soul/rollback",
            post(handlers::soul::rollback_soul),
        )
        .route(
            "/bots/{id}/soul/verify",
            get(handlers::soul::verify_soul),
        )
        // Chat streaming
        .route(
            "/bots/{id}/chat/stream",
            post(handlers::chat::stream_chat),
        )
        // Sessions (bot-scoped)
        .route(
            "/bots/{id}/sessions",
            get(handlers::session::list_sessions),
        )
        // Sessions (top-level)
        .route(
            "/sessions/{id}",
            get(handlers::session::get_session),
        )
        .route(
            "/sessions/{id}",
            delete(handlers::session::delete_session),
        )
        .route(
            "/sessions/{id}/messages",
            get(handlers::session::get_messages),
        )
        .route(
            "/sessions/{id}/clear",
            post(handlers::session::clear_session),
        )
        // Identity / User file management
        .route(
            "/bots/{id}/identity",
            get(handlers::identity::get_identity).put(handlers::identity::update_identity),
        )
        .route(
            "/bots/{id}/user",
            get(handlers::identity::get_user_context).put(handlers::identity::update_user_context),
        )
        // Dashboard stats
        .route("/stats", get(handlers::stats::get_stats))
        // Secrets
        .route("/secrets", get(handlers::secret::list_secrets))
        .route("/secrets/{key}", put(handlers::secret::set_secret))
        .route("/secrets/{key}", delete(handlers::secret::delete_secret))
        // Skills (global library)
        .route("/skills", get(handlers::skill::list_skills))
        .route("/skills/{name}", get(handlers::skill::get_skill))
        .route(
            "/skills/install",
            post(handlers::skill::install_skill),
        )
        // Skills (bot-scoped)
        .route(
            "/bots/{id}/skills",
            get(handlers::skill::list_bot_skills).post(handlers::skill::attach_skill),
        )
        .route(
            "/bots/{id}/skills/{name}",
            delete(handlers::skill::detach_skill).patch(handlers::skill::update_skill_config),
        )
        // Registry
        .route(
            "/registry/search",
            get(handlers::skill::search_registry),
        )
        // Builder sessions
        .route(
            "/builder/sessions",
            post(handlers::builder::create_builder_session),
        )
        .route(
            "/builder/sessions/{session_id}/answer",
            post(handlers::builder::submit_answer),
        )
        .route(
            "/builder/sessions/{session_id}/assemble",
            post(handlers::builder::assemble_bot),
        )
        .route(
            "/builder/sessions/{session_id}/create-skill",
            post(handlers::builder::create_skill),
        )
        .route(
            "/builder/sessions/{session_id}",
            get(handlers::builder::get_session).delete(handlers::builder::delete_session),
        )
        .route(
            "/builder/sessions/{session_id}/reconfigure",
            post(handlers::builder::reconfigure_bot),
        )
        .route(
            "/builder/drafts",
            get(handlers::builder::list_drafts),
        );

    let mut router = Router::new()
        .nest("/api/v1", api_routes)
        .route("/health", get(health_check))
        // WebSocket for real-time agent events and bidirectional commands.
        // Outside /api/v1 since WebSocket is not a REST endpoint.
        .route("/ws/events", get(handlers::ws::ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Serve the built React SPA from disk if the directory exists.
    // API routes and /health take priority; unknown paths fall through
    // to index.html for client-side routing.
    let web_dir =
        std::env::var("BOTERNITY_WEB_DIR").unwrap_or_else(|_| "apps/web/dist".to_string());
    if std::path::Path::new(&web_dir).exists() {
        let index_path = format!("{}/index.html", web_dir);
        let serve_dir = ServeDir::new(&web_dir).fallback(ServeFile::new(index_path));
        router = router.fallback_service(serve_dir);
        tracing::info!(path = %web_dir, "SPA static file serving enabled");
    }

    router
}

/// GET /health - Simple health check endpoint (no auth required).
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
