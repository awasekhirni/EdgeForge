// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::{
    routes::{chat, completions, embeddings, health, models},
    state::AppState,
};
use axum::Router;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::info;
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(models::routes())
        .merge(chat::routes())
        .merge(completions::routes())
        .merge(embeddings::routes())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
pub async fn serve(
    addr: SocketAddr,
    state: AppState,
) -> Result<(), edgeforge_core::EdgeForgeError> {
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        edgeforge_core::EdgeForgeError::Io {
            path: addr.to_string().into(),
            source: e,
        }
    })?;
    info!(%addr, "edgeforge API server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| edgeforge_core::EdgeForgeError::Engine(format!("server stopped: {e}")))?;
    Ok(())
}
