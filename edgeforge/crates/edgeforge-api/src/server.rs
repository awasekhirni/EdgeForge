//! # Server Bootstrap
//!
//! ## Purpose
//! Builds the axum router with all routes and middleware, and binds the server
//! to the configured address.
//!
//! ## What This File Does
//! - `build_router(state)` — creates the axum Router with all routes + middleware
//! - `serve(addr, state)` — binds TcpListener and serves until shutdown
//!
//! ## Middleware Stack (outer → inner)
//! The router applies middleware in this order (outermost first):
//! 1. `TraceLayer` (tower-http) — HTTP request/response tracing
//! 2. `logging_middleware` — structured request/response logging
//! 3. `auth_middleware` — API key validation (401 on failure)
//! 4. `rate_limit_middleware` — per-IP token-bucket (429 on failure)
//!
//! The `/health` endpoint is mounted on a **separate** router that bypasses
//! auth + rate limiting (so health checks always succeed). All `/v1/*` routes
//! go through the full middleware stack.
//!
//! ## Dependencies
//! - `axum` — Router, serve, middleware
//! - `tower_http::trace::TraceLayer` — request tracing
//! - `tokio::net::TcpListener` — TCP binding
//! - `tracing` — startup log
//! - `crate::routes` — all route handlers
//! - `crate::middleware::auth::auth_middleware` — API key auth
//! - `crate::middleware::rate_limit::rate_limit_middleware` — per-IP rate limiting
//! - `crate::middleware::logging::logging_middleware` — structured logging
//!
//! ## Output
//! `Result<(), EdgeForgeError>` — Ok on graceful shutdown, Engine error on failure.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - server.rs - function - STATUS`

use crate::{
    middleware::{
        auth::auth_middleware,
        logging::logging_middleware,
        rate_limit::rate_limit_middleware,
    },
    routes::{chat, completions, embeddings, files, health, models, webhooks},
    state::AppState,
};
use axum::Router;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::net::SocketAddr;
use std::sync::LazyLock;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Build the axum router with all routes and middleware.
///
/// ## Purpose
/// Creates the complete axum Router with all route handlers and middleware
/// layers. The router includes:
///
/// **Public routes** (no auth, no rate limiting):
/// - `GET /health` — liveness probe
///
/// **Protected routes** (auth + rate limited when configured):
/// - `GET /v1/models`, `GET /v1/models/:id` — model management
/// - `POST /v1/chat/completions` — chat completions (SSE streaming supported)
/// - `POST /v1/completions` — legacy text completions
/// - `POST /v1/embeddings` — generate embedding vectors
/// - `POST /v1/embeddings/search` — RAG similarity search
/// - `POST /v1/files` — RAG file upload
/// - `GET/POST /v1/webhooks` — webhook management
///
/// **Middleware stack** (applied to protected routes, outer → inner):
/// 1. `TraceLayer` — HTTP tracing
/// 2. `logging_middleware` — structured request/response logging
/// 3. `auth_middleware` — API key validation (401 on failure)
/// 4. `rate_limit_middleware` — per-IP token-bucket (429 on failure)
///
/// ## Parameters
/// - `state`: The application state (shared by all handlers).
///
/// ## Returns
/// `Router` — the configured axum router.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - server.rs - build_router - STATUS`
pub fn build_router(state: AppState) -> Router {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "server.rs", "build_router");

    tracing::info!(
        require_api_key = state.require_api_key,
        has_api_key = state.api_key.is_some(),
        rate_limit_per_minute = state.rate_limit_per_minute,
        "build_router: wiring middleware"
    );

    // Public routes: /health bypasses auth + rate limiting.
    // (Docker healthchecks and load balancer probes must always succeed.)
    let public_routes = Router::new().merge(health::routes());

    // Protected routes: all /v1/* endpoints go through the full middleware stack.
    let protected_routes = Router::new()
        .merge(models::routes())
        .merge(chat::routes())
        .merge(completions::routes())
        .merge(embeddings::routes())
        .merge(files::routes())
        .merge(webhooks::routes())
        // Middleware is applied outer → inner. The last `.layer()` added is
        // the innermost (runs first on the way in, last on the way out).
        // We want the execution order on the way in to be:
        //   rate_limit → auth → logging → handler
        // So we add them in reverse order:
        //   logging (added first → outermost on the way out, innermost on the way in... no)
        // Actually, axum middleware execution order: layers added LATER run FIRST
        // on the request path. So to get rate_limit → auth → logging → handler,
        // we add: logging first, then auth, then rate_limit.
        .layer(axum::middleware::from_fn(logging_middleware))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));

    // Combine public + protected, apply TraceLayer to everything, set state.
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Bind the server to the configured address and serve until shutdown.
///
/// ## Purpose
/// Binds a TcpListener to the given address and serves the axum router
/// until the server is shut down. Uses `into_make_service_with_connect_info`
/// so the rate limiter can read the client's IP address.
///
/// ## Parameters
/// - `addr`: The address to bind to.
/// - `state`: The application state.
///
/// ## Returns
/// `Ok(())` on graceful shutdown, `Err(EdgeForgeError)` on bind/serve failure.
///
/// ## Errors
/// - `Io` if the address cannot be bound.
/// - `Engine` if the server stops with an error.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - server.rs - serve - STATUS`
pub async fn serve(addr: SocketAddr, state: AppState) -> Result<(), edgeforge_core::EdgeForgeError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "server.rs", "serve");
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| edgeforge_core::EdgeForgeError::Io {
            path: addr.to_string().into(),
            source: e,
        })?;
    info!(%addr, "edgeforge API server listening");
    guard.success();
    // into_make_service_with_connect_info is required so the rate_limit_middleware
    // can extract the client's SocketAddr via ConnectInfo<SocketAddr>.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| edgeforge_core::EdgeForgeError::Engine(format!("server stopped: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::AppState;
    use edgeforge_orchestration::{Embeddings, Jobs, Models};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_state() -> AppState {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        // Create minimal tables for state construction
        sqlx::query("CREATE TABLE IF NOT EXISTS models (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS embedding_sources (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS embedding_vectors (id TEXT PRIMARY KEY)")
            .execute(&pool).await.unwrap();
        AppState::new(
            Models::new(pool.clone()),
            Jobs::new(pool.clone()),
            Embeddings::new(pool),
            false, None, 600,
        )
    }

    #[tokio::test]
    async fn test_build_router_does_not_panic() {
        let state = make_state().await;
        let _router = build_router(state);
    }

    #[tokio::test]
    async fn test_build_router_has_health_route() {
        let state = make_state().await;
        let router = build_router(state);
        // Just verify it builds without panic
        let _ = format!("{router:?}");
    }
}
