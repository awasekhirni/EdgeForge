// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # GET /v1/models and GET /v1/models/:id
//!
//! ## Purpose
//! List all imported models or fetch a specific model by ID.
//!
//! ## What This File Does
//! - `list_models()` — calls state.models.list(), returns ModelsResponse
//! - `get_model()` — calls state.models.get(), returns ModelInfo
//!
//! ## Dependencies
//! - `edgeforge-orchestration::Models` — via AppState
//! - `edgeforge-core::ModelId` — for ID parsing
//! - `crate::{dto, middleware::ApiError, state::AppState}`
//!
//! ## Output
//! `Json<ModelsResponse>` or `Json<ModelInfo>`.
//!
//! ## Function: list_models
//! - **Purpose:** List all imported models, most-recently-used first
//! - **Returns:** `Result<Json<ModelsResponse>, ApiError>`
//! - **Dependencies:** `state.models.list()`

use crate::{
    dto::{ModelInfo, ModelsResponse},
    middleware::ApiError,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
/// Returns the axum Router for this module's routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/models/:id", get(get_model))
}
async fn list_models(State(state): State<AppState>) -> Result<Json<ModelsResponse>, ApiError> {
    let rows = state.models.list().await?;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let data = rows
        .into_iter()
        .map(|r| ModelInfo {
            id: r.id,
            object: "model",
            created,
            owned_by: r.publisher,
        })
        .collect();
    Ok(Json(ModelsResponse {
        object: "list",
        data,
    }))
}
async fn get_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ModelInfo>, ApiError> {
    let mid = edgeforge_core::ModelId::parse(&id)
        .map_err(|e| edgeforge_core::EdgeForgeError::InvalidRequest(e.to_string()))?;
    let row = state
        .models
        .get(mid)
        .await?
        .ok_or_else(|| edgeforge_core::EdgeForgeError::ModelNotFound(id.clone()))?;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(Json(ModelInfo {
        id: row.id,
        object: "model",
        created,
        owned_by: row.publisher,
    }))
}
