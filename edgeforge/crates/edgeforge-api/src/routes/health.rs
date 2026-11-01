// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::dto::HealthResponse;
use axum::{routing::get, Json, Router};
pub fn routes() -> Router<crate::AppState> {
    Router::new().route("/health", get(health))
}
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
