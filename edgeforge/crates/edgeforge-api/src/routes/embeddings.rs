//! # POST /v1/embeddings — RAG Embeddings Endpoint (F-DEV-06)
//!
//! ## Purpose
//! Generates real embedding vectors for text input using the
//! `EmbeddingEngine` in `AppState` (default: `HashingEmbedder`). Used for
//! RAG (Retrieval-Augmented Generation) to embed documents and queries into
//! a vector space. Also exposes a similarity-search endpoint for retrieving
//! the top-k most similar stored chunks.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for this module's endpoints
//! - `embeddings()` — handler for POST /v1/embeddings (generate vectors)
//! - `search()` — handler for POST /v1/embeddings/search (similarity search)
//! - `EmbeddingsRequest` — request DTO (model, input)
//! - `EmbeddingsResponse` — response DTO (data, model, usage)
//! - `EmbeddingData` — single embedding (index, embedding, object)
//! - `SearchRequest` — request DTO for similarity search
//! - `SearchResponse` — response DTO for similarity search
//!
//! ## Endpoints
//! - `POST /v1/embeddings` — embed one or more texts
//! - `POST /v1/embeddings/search` — find the top-k most similar stored chunks
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError
//! - `edgeforge-runtime` — EmbeddingEngine
//! - `edgeforge-orchestration` — Embeddings ORM, StoreChunkParams
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing
//! - `serde` — request/response serialization
//!
//! ## Output
//! JSON response with real (non-zero) embedding vectors, or search results.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - embeddings.rs - function - STATUS`

use crate::{middleware::ApiError, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use edgeforge_core::EdgeForgeError;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use edgeforge_orchestration::StoreChunkParams;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Request body for POST /v1/embeddings.
#[derive(Clone, Debug, Deserialize)]
pub struct EmbeddingsRequest {
    /// The embedding model ID. This is stored alongside the vectors so queries
    /// can filter by model. If it doesn't match the configured embedder's
    /// model_id, the embedder's model_id is used instead (and a warning is logged).
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
    /// The embedding vector (real, non-zero, L2-normalised).
    pub embedding: Vec<f32>,
}

/// Token usage for embeddings.
#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingsUsage {
    /// Tokens in the prompt (approximated by word count).
    pub prompt_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// Request body for POST /v1/embeddings/search.
#[derive(Clone, Debug, Deserialize)]
pub struct SearchRequest {
    /// The embedding model ID to search within.
    pub model: String,
    /// The query text to embed and search for.
    pub query: String,
    /// Maximum number of results to return (default 5, max 100).
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    5
}

/// Response body for POST /v1/embeddings/search.
#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    /// Object type ("list").
    pub object: &'static str,
    /// The model ID used for the query.
    pub model: String,
    /// Search results, ranked by cosine similarity descending.
    pub results: Vec<SearchResultDto>,
}

/// A single search result DTO.
#[derive(Clone, Debug, Serialize)]
pub struct SearchResultDto {
    /// The source ID.
    pub source_id: String,
    /// The source URI (file path, URL, etc.).
    pub source_uri: String,
    /// Chunk index within the source.
    pub chunk_idx: Option<i64>,
    /// The text content of the matched chunk.
    pub content: String,
    /// Cosine similarity score in [-1, 1].
    pub score: f32,
    /// The embedding model ID.
    pub model_id: String,
}

/// Returns the axum Router for the embeddings endpoints.
///
/// ## Routes
/// - `POST /v1/embeddings` — embed one or more texts
/// - `POST /v1/embeddings/search` — similarity search across stored chunks
///
/// ## Function Logging
/// `timestamp - edgeforge-api - embeddings.rs - routes - STATUS`
pub fn routes() -> Router<AppState> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "embeddings.rs", "routes");
    Router::new()
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/embeddings/search", post(search))
}

/// Handler for POST /v1/embeddings.
///
/// ## Purpose
/// Generates real embedding vectors for the input text(s) using the configured
/// `EmbeddingEngine` (default: `HashingEmbedder`). The vectors are
/// L2-normalised and suitable for cosine-similarity comparison.
///
/// ## Parameters
/// - `state`: The application state (contains the embedder).
/// - `req`: The embeddings request body.
///
/// ## Returns
/// `Json<EmbeddingsResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - Never returns 501 — the HashingEmbedder always produces real vectors.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - embeddings.rs - embeddings - STATUS`
async fn embeddings(
    State(state): State<AppState>,
    Json(req): Json<EmbeddingsRequest>,
) -> Result<Json<EmbeddingsResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "embeddings.rs", "embeddings");

    // Determine the model_id to use. If the request's model doesn't match the
    // embedder's model_id, we use the embedder's model_id (the embedder is
    // what actually produces the vectors).
    let embedder_model_id = state.embedder.model_id().to_string();
    if req.model != embedder_model_id {
        tracing::info!(
            requested_model = %req.model,
            actual_model = %embedder_model_id,
            "Embeddings: requested model differs from configured embedder — using embedder"
        );
    }

    // Determine the number of inputs.
    let inputs: Vec<String> = match req.input {
        EmbeddingsInput::Single(s) => vec![s],
        EmbeddingsInput::Many(v) => v,
    };

    let input_count = inputs.len() as u32;
    let total_tokens: u32 = inputs.iter().map(|s| s.split_whitespace().count() as u32).sum();

    tracing::info!(
        model = %embedder_model_id,
        input_count,
        total_tokens,
        "Embeddings: generating vectors"
    );

    // Generate real embeddings using the configured embedder.
    let data: Vec<EmbeddingData> = inputs
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let vector = state.embedder.embed(text);
            EmbeddingData {
                object: "embedding",
                index: i as u32,
                embedding: vector,
            }
        })
        .collect();

    let response = EmbeddingsResponse {
        object: "list",
        data,
        model: embedder_model_id,
        usage: EmbeddingsUsage {
            prompt_tokens: total_tokens,
            total_tokens,
        },
    };

    tracing::info!(count = input_count, tokens = total_tokens, "Embeddings: vectors generated");
    guard.success();
    Ok(Json(response))
}

/// Handler for POST /v1/embeddings/search.
///
/// ## Purpose
/// Embeds the query text, then searches the `embedding_vectors` table for the
/// top-k most similar stored chunks (cosine similarity). Results are ranked
/// by similarity, descending.
///
/// ## Parameters
/// - `state`: The application state (contains the embedder + embeddings ORM).
/// - `req`: The search request body.
///
/// ## Returns
/// `Json<SearchResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - `Database` if the search query fails.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - embeddings.rs - search - STATUS`
async fn search(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "embeddings.rs", "search");

    let embedder_model_id = state.embedder.model_id().to_string();
    let top_k = req.top_k.min(100).max(1);

    tracing::info!(
        model = %embedder_model_id,
        query_len = req.query.len(),
        top_k,
        "Embeddings: search request"
    );

    // Embed the query.
    let query_vec = state.embedder.embed(&req.query);

    // Search the vector store.
    let results = state
        .embeddings
        .search(&query_vec, &embedder_model_id, top_k)
        .await?;

    let result_dtos: Vec<SearchResultDto> = results
        .into_iter()
        .map(|r| SearchResultDto {
            source_id: r.source_id,
            source_uri: r.source_uri,
            chunk_idx: r.chunk_idx,
            content: r.content,
            score: r.score,
            model_id: r.model_id,
        })
        .collect();

    let result_count = result_dtos.len();
    let response = SearchResponse {
        object: "list",
        model: embedder_model_id,
        results: result_dtos,
    };

    tracing::info!(result_count, "Embeddings: search complete");
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

    #[test]
    fn test_search_request_default_top_k() {
        let json = r#"{"model":"test","query":"hello"}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.top_k, 5);
    }

    #[test]
    fn test_search_request_custom_top_k() {
        let json = r#"{"model":"test","query":"hello","top_k":10}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.top_k, 10);
    }

    #[test]
    fn test_search_response_serialize() {
        let resp = SearchResponse {
            object: "list",
            model: "test".to_string(),
            results: vec![SearchResultDto {
                source_id: "src-1".to_string(),
                source_uri: "/path/to/file.txt".to_string(),
                chunk_idx: Some(0),
                content: "hello world".to_string(),
                score: 0.95,
                model_id: "test".to_string(),
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("src-1"));
        assert!(json.contains("0.95"));
        assert!(json.contains("hello world"));
    }
}
