// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use axum::{routing::post, Json, Router};
pub fn routes() -> Router<crate::AppState> {
    Router::new().route("/v1/completions", post(completions))
}
async fn completions() -> Result<Json<serde_json::Value>, crate::middleware::ApiError> {
    Err(edgeforge_core::EdgeForgeError::NotImplemented("completions (Phase 3)").into())
}
