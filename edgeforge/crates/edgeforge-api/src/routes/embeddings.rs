// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use axum::{routing::post, Json, Router};
pub fn routes() -> Router<crate::AppState> {
    Router::new().route("/v1/embeddings", post(embeddings))
}
async fn embeddings() -> Result<Json<serde_json::Value>, crate::middleware::ApiError> {
    Err(edgeforge_core::EdgeForgeError::NotImplemented("embeddings (Phase 3)").into())
}
