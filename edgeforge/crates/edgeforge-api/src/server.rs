// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
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
//! ## Dependencies
//! - `axum` — Router, serve
//! - `tower_http::trace::TraceLayer` — request tracing
//! - `tokio::net::TcpListener` — TCP binding
//! - `tracing` — startup log
//! - `crate::routes` — all route handlers
//! - `crate::middleware::logging` — structured request/response logging
//!
//! ## Output
//! `Result<(), EdgeForgeError>` — Ok on graceful shutdown, Engine error on failure.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - server.rs - function - STATUS`

use crate::{
    middleware::logging::logging_middleware,
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
/// - Health check (GET /health)
/// - Model management (GET /v1/models, GET /v1/models/:id)
/// - Chat completions (POST /v1/chat/completions)
/// - Text completions (POST /v1/completions)
/// - Embeddings (POST /v1/embeddings)
/// - File upload (POST /v1/files)
/// - Webhooks (GET/POST /v1/webhooks)
/// - Request/response logging middleware
/// - HTTP tracing middleware
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
    Router::new()
        .merge(health::routes())
        .merge(models::routes())
        .merge(chat::routes())
        .merge(completions::routes())
        .merge(embeddings::routes())
        .merge(files::routes())
        .merge(webhooks::routes())
        .layer(axum::middleware::from_fn(logging_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Bind the server to the configured address and serve until shutdown.
///
/// ## Purpose
/// Binds a TcpListener to the given address and serves the axum router
/// until the server is shut down.
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
pub async fn serve(
    addr: SocketAddr,
    state: AppState,
) -> Result<(), edgeforge_core::EdgeForgeError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "server.rs", "serve");
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        edgeforge_core::EdgeForgeError::Io {
            path: addr.to_string().into(),
            source: e,
        }
    })?;
    info!(%addr, "edgeforge API server listening");
    guard.success();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| edgeforge_core::EdgeForgeError::Engine(format!("server stopped: {e}")))?;
    Ok(())
}
