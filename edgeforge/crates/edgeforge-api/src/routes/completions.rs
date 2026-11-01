// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # POST /v1/completions — Legacy Text Completion Endpoint
//!
//! ## Purpose
//! Handles legacy text completion requests (not chat-formatted). Similar to
//! the OpenAI /v1/completions endpoint.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for this endpoint
//! - `completions()` — handler for POST /v1/completions
//! - `CompletionRequest` — request DTO
//! - `CompletionResponse` — response DTO
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing
//! - `serde` — request/response serialization
//!
//! ## Output
//! JSON response with completion text, or 501 if not implemented.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - completions.rs - function - STATUS`

use crate::{middleware::ApiError, state::AppState};
use axum::{routing::post, Json, Router};
use edgeforge_core::EdgeForgeError;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Request body for POST /v1/completions.
#[derive(Clone, Debug, Deserialize)]
pub struct CompletionRequest {
    /// The model ID to use for completion.
    pub model: String,
    /// The prompt text to complete.
    pub prompt: String,
    /// Maximum number of tokens to generate.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Sampling temperature.
    #[serde(default = "default_temp")]
    pub temperature: f32,
}

fn default_max_tokens() -> u32 {
    128
}

fn default_temp() -> f32 {
    0.7
}

/// Response body for POST /v1/completions.
#[derive(Clone, Debug, Serialize)]
pub struct CompletionResponse {
    /// Unique completion ID.
    pub id: String,
    /// Object type ("text_completion").
    pub object: &'static str,
    /// Creation timestamp (Unix seconds).
    pub created: i64,
    /// Model ID used.
    pub model: String,
    /// Completion choices.
    pub choices: Vec<CompletionChoice>,
    /// Token usage.
    pub usage: CompletionUsage,
}

/// A single completion choice.
#[derive(Clone, Debug, Serialize)]
pub struct CompletionChoice {
    /// Choice index.
    pub index: u32,
    /// The completed text.
    pub text: String,
    /// Finish reason ("stop", "length", etc.).
    pub finish_reason: Option<String>,
}

/// Token usage statistics.
#[derive(Clone, Debug, Serialize)]
pub struct CompletionUsage {
    /// Tokens in the prompt.
    pub prompt_tokens: u32,
    /// Tokens generated.
    pub completion_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// Returns the axum Router for the completions endpoint.
///
/// ## Returns
/// `Router<AppState>` — the router with POST /v1/completions.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - completions.rs - routes - STATUS`
pub fn routes() -> Router<AppState> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "completions.rs", "routes");
    Router::new().route("/v1/completions", post(completions))
}

/// Handler for POST /v1/completions.
///
/// ## Purpose
/// Processes a text completion request. Currently returns a stub response
/// indicating the endpoint is not yet wired to the inference engine.
///
/// ## Parameters
/// - `state`: The application state.
/// - `req`: The completion request body.
///
/// ## Returns
/// `Json<CompletionResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - 501 Not Implemented — the endpoint is not yet wired to the inference engine.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - completions.rs - completions - STATUS`
async fn completions(
    State(_state): State<AppState>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "completions.rs", "completions");
    tracing::info!(model = %req.model, prompt_len = req.prompt.len(), "Completion request");

    // Phase 11: Return a stub response. The real implementation will use
    // the InferenceEngine to generate completions.
    let created = chrono::Utc::now().timestamp();
    let response = CompletionResponse {
        id: format!("cmpl-{}", edgeforge_core::ModelId::new()),
        object: "text_completion",
        created,
        model: req.model,
        choices: vec![CompletionChoice {
            index: 0,
            text: "Text completion endpoint — Phase 11 stub (wire to InferenceEngine in Phase 12)"
                .to_string(),
            finish_reason: Some("stop".to_string()),
        }],
        usage: CompletionUsage {
            prompt_tokens: req.prompt.split_whitespace().count() as u32,
            completion_tokens: 1,
            total_tokens: req.prompt.split_whitespace().count() as u32 + 1,
        },
    };

    guard.success();
    Ok(Json(response))
}

use axum::extract::State;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request_deserialize() {
        let json = r#"{"model":"test","prompt":"hello","max_tokens":50,"temperature":0.5}"#;
        let req: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "test");
        assert_eq!(req.prompt, "hello");
        assert_eq!(req.max_tokens, 50);
    }

    #[test]
    fn test_completion_request_defaults() {
        let json = r#"{"model":"test","prompt":"hello"}"#;
        let req: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_tokens, 128);
        assert_eq!(req.temperature, 0.7);
    }

    #[test]
    fn test_completion_response_serialize() {
        let resp = CompletionResponse {
            id: "cmpl-123".to_string(),
            object: "text_completion",
            created: 1234567890,
            model: "test".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                text: "hello world".to_string(),
                finish_reason: Some("stop".to_string()),
            }],
            usage: CompletionUsage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("text_completion"));
        assert!(json.contains("hello world"));
    }
}
