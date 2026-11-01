// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # GET /health — Liveness Probe
//!
//! ## Purpose
//! Returns server status and version. Used by Docker healthcheck and
//! load balancers.
//!
//! ## What This File Does
//! - `health()` — returns HealthResponse with status="ok" and version
//!
//! ## Dependencies
//! - `axum` — routing
//!
//! ## Output
//! `Json<HealthResponse>` — `{"status":"ok","version":"0.1.0"}`

use crate::dto::HealthResponse;
use axum::{routing::get, Json, Router};
/// Returns the axum Router for this module's routes.
pub fn routes() -> Router<crate::AppState> {
    Router::new().route("/health", get(health))
}
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
