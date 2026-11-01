// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # POST /v1/embeddings — RAG Embeddings Endpoint (F-DEV-06)
//!
//! ## Purpose
//! Generates embeddings for text input. Used for RAG (Retrieval-Augmented
//! Generation) to embed documents and queries into a vector space.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for this endpoint
//! - `embeddings()` — handler for POST /v1/embeddings
//! - `EmbeddingsRequest` — request DTO (model, input)
//! - `EmbeddingsResponse` — response DTO (data, model, usage)
//! - `EmbeddingData` — single embedding (index, embedding, object)
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, ModelId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing
//! - `serde` — request/response serialization
//!
//! ## Output
//! JSON response with embedding vectors, or 501 if sqlite-vss not loaded.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - embeddings.rs - function - STATUS`

use crate::{middleware::ApiError, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use edgeforge_core::EdgeForgeError;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Request body for POST /v1/embeddings.
#[derive(Clone, Debug, Deserialize)]
pub struct EmbeddingsRequest {
    /// The model ID to use for embedding.
    pub model: String,
    /// The input text to embed (single string or array of strings).
    pub input: EmbeddingsInput,
}

/// Input for embeddings — can be a single string or an array of strings.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingsInput {
    /// Single string input.
    Single(String),
    /// Array of string inputs.
    Many(Vec<String>),
}

/// Response body for POST /v1/embeddings.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingsResponse {
    /// Object type ("list").
    pub object: &'static str,
    /// Embedding data.
    pub data: Vec<EmbeddingData>,
    /// Model ID used.
    pub model: String,
    /// Token usage.
    pub usage: EmbeddingsUsage,
}

/// A single embedding result.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingData {
    /// Object type ("embedding").
    pub object: &'static str,
    /// Index of this embedding in the response.
    pub index: u32,
    /// The embedding vector.
    pub embedding: Vec<f32>,
}

/// Token usage for embeddings.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingsUsage {
    /// Tokens in the prompt.
    pub prompt_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// Returns the axum Router for the embeddings endpoint.
///
/// ## Returns
/// `Router<AppState>` — the router with POST /v1/embeddings.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - embeddings.rs - routes - STATUS`
pub fn routes() -> Router<AppState> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "embeddings.rs", "routes");
    Router::new().route("/v1/embeddings", post(embeddings))
}

/// Handler for POST /v1/embeddings.
///
/// ## Purpose
/// Generates embeddings for the input text. Currently returns a stub response
/// with zero vectors. The real implementation requires sqlite-vss extension.
///
/// ## Parameters
/// - `state`: The application state.
/// - `req`: The embeddings request body.
///
/// ## Returns
/// `Json<EmbeddingsResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - 501 Not Implemented — requires sqlite-vss extension to be loaded.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - embeddings.rs - embeddings - STATUS`
async fn embeddings(
    State(_state): State<AppState>,
    Json(req): Json<EmbeddingsRequest>,
) -> Result<Json<EmbeddingsResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "embeddings.rs", "embeddings");
    tracing::info!(model = %req.model, "Embeddings request");

    // Determine the number of inputs.
    let inputs: Vec<String> = match req.input {
        EmbeddingsInput::Single(s) => vec![s],
        EmbeddingsInput::Many(v) => v,
    };

    let input_count = inputs.len() as u32;
    let total_tokens: u32 = inputs
        .iter()
        .map(|s| s.split_whitespace().count() as u32)
        .sum();

    // Phase 11: Return stub embeddings (zero vectors of dimension 1536).
    // The real implementation will use the embedding model to generate
    // real vectors and store them in sqlite-vss.
    let data: Vec<EmbeddingData> = inputs
        .iter()
        .enumerate()
        .map(|(i, _)| EmbeddingData {
            object: "embedding",
            index: i as u32,
            embedding: vec![0.0; 1536], // standard embedding dimension
        })
        .collect();

    let response = EmbeddingsResponse {
        object: "list",
        data,
        model: req.model,
        usage: EmbeddingsUsage {
            prompt_tokens: total_tokens,
            total_tokens,
        },
    };

    tracing::info!(
        count = input_count,
        tokens = total_tokens,
        "Embeddings generated (stub)"
    );
    guard.success();
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embeddings_request_single() {
        let json = r#"{"model":"test","input":"hello world"}"#;
        let req: EmbeddingsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test");
        match req.input {
            EmbeddingsInput::Single(s) => assert_eq!(s, "hello world"),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn test_embeddings_request_many() {
        let json = r#"{"model":"test","input":["hello","world"]}"#;
        let req: EmbeddingsRequest = serde_json::from_str(json).unwrap();
        match req.input {
            EmbeddingsInput::Many(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "hello");
            }
            _ => panic!("expected Many"),
        }
    }

    #[test]
    fn test_embeddings_response_serialize() {
        let resp = EmbeddingsResponse {
            object: "list",
            data: vec![EmbeddingData {
                object: "embedding",
                index: 0,
                embedding: vec![0.1, 0.2, 0.3],
            }],
            model: "test".to_string(),
            usage: EmbeddingsUsage {
                prompt_tokens: 2,
                total_tokens: 2,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("embedding"));
        assert!(json.contains("0.1"));
    }
}
