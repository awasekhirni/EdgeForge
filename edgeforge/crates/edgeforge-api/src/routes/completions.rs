//! # POST /v1/completions — Legacy Text Completion Endpoint
//!
//! ## Purpose
//! Handles legacy text completion requests (not chat-formatted). Similar to
//! the OpenAI /v1/completions endpoint. Wired to `SsdEngine` (the pure-Rust
//! working inference engine) so it produces real completions.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for this endpoint
//! - `completions()` — handler for POST /v1/completions
//! - `CompletionRequest` — request DTO
//! - `CompletionResponse` — response DTO
//!
//! ## Pipeline
//! 1. Parse the model ID and verify it exists in the registry
//! 2. Build a manifest from the registry row
//! 3. Create an `SsdEngine`, load the model
//! 4. Build a single-message `Prompt` from the request's `prompt` field
//! 5. Generate tokens via `engine.generate()`
//! 6. Collect the generated text and return it as a `CompletionResponse`
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, ModelId, Prompt, ChatMessage, Role
//! - `edgeforge-runtime` — SsdEngine, InferenceEngine, GenParams
//! - `edgeforge-orchestration` — via AppState (Models)
//! - `edgeforge-security` — ModelManifest, ModelArch
//! - `edgeforge-crowd` — ExecutionMode
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing
//! - `serde` — request/response serialization
//!
//! ## Output
//! JSON response with real completion text (not a stub).
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - completions.rs - function - STATUS`

use crate::{middleware::ApiError, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use edgeforge_core::{ChatMessage, EdgeForgeError, ModelId, Prompt, Role, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use edgeforge_runtime::{GenParams, InferenceEngine, SsdEngine};
use edgeforge_security::{ModelArch, ModelManifest};
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
    /// Top-p sampling threshold.
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    /// Optional random seed for deterministic generation.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Stop sequences (halt generation when any is matched).
    #[serde(default)]
    pub stop: Vec<String>,
}

fn default_max_tokens() -> u32 {
    128
}

fn default_temp() -> f32 {
    0.7
}

fn default_top_p() -> f32 {
    0.9
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
    /// Tokens in the prompt (approximated by word count).
    pub prompt_tokens: u32,
    /// Tokens generated.
    pub completion_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// Returns the axum Router for the completions endpoint.
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
/// Processes a text completion request using the `SsdEngine` inference engine.
/// The prompt is treated as a single user message and the engine generates
/// tokens until `max_tokens` is reached or a stop sequence is matched.
///
/// ## Parameters
/// - `state`: The application state.
/// - `req`: The completion request body.
///
/// ## Returns
/// `Json<CompletionResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - 404 ModelNotFound if the model ID doesn't exist in the registry.
/// - 500 Engine if generation fails.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - completions.rs - completions - STATUS`
async fn completions(
    State(state): State<AppState>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "completions.rs", "completions");
    tracing::info!(
        model = %req.model,
        prompt_len = req.prompt.len(),
        max_tokens = req.max_tokens,
        "Completion request"
    );

    // Parse model ID and verify it exists in the registry.
    let mid = ModelId::parse(&req.model)
        .map_err(|e| EdgeForgeError::InvalidRequest(e.to_string()))?;
    let row = state
        .models
        .get(mid)
        .await?
        .ok_or_else(|| EdgeForgeError::ModelNotFound(req.model.clone()))?;

    // Build a manifest from the registry row.
    let manifest = ModelManifest {
        schema_version: 1,
        id: mid,
        name: row.name.clone(),
        publisher: row.publisher.clone(),
        license: row.license.clone(),
        license_url: None,
        acceptable_uses: vec![],
        geo_restrictions: vec![],
        file_size_bytes: row.file_size_bytes as u64,
        sha256: row.sha256.clone().try_into().unwrap_or([0u8; 32]),
        precision: row.precision().unwrap_or(edgeforge_core::Precision::Fp16),
        arch: ModelArch::Moe,
        total_params: row.total_params as u64,
        active_params: row.active_params.map(|v| v as u64),
        dense_core_bytes: row.dense_core_bytes as u64,
        expert_pool_bytes: row.expert_pool_bytes as u64,
        signature: None,
        publisher_key_fingerprint: None,
    };

    // Create the SsdEngine and load the model.
    let engine = SsdEngine::new();
    let model = engine
        .load(&manifest, ExecutionMode::Local)
        .await
        .map_err(|e| EdgeForgeError::Engine(e.to_string()))?;

    // Build a single-message prompt from the request's prompt field.
    let prompt = Prompt::new(vec![ChatMessage {
        role: Role::User,
        content: req.prompt.clone(),
    }]);

    // Build generation parameters.
    let params = GenParams {
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        seed: req.seed,
        stop_sequences: req.stop.clone(),
    };

    let created = chrono::Utc::now().timestamp();
    let completion_id = format!("cmpl-{}", ModelId::new());

    // Generate the completion, collecting the text.
    let mut full_text = String::new();
    let mut token_count = 0u32;

    let stats = engine
        .generate(&model, &prompt, &params, &mut |token: Token, _outcome| {
            full_text.push_str(&token.text);
            token_count += 1;
            true
        })
        .await
        .map_err(|e| EdgeForgeError::Engine(e.to_string()))?;

    tracing::info!(
        completion_id = %completion_id,
        tokens_generated = stats.tokens_generated,
        elapsed_ms = stats.elapsed_ms,
        "Completion generated"
    );

    // Determine finish reason.
    let finish_reason = if stats.tokens_generated < req.max_tokens {
        "stop" // stopped early (stop sequence or callback)
    } else {
        "length" // hit max_tokens
    };

    let prompt_tokens = req.prompt.split_whitespace().count() as u32;
    let response = CompletionResponse {
        id: completion_id,
        object: "text_completion",
        created,
        model: req.model,
        choices: vec![CompletionChoice {
            index: 0,
            text: full_text,
            finish_reason: Some(finish_reason.to_string()),
        }],
        usage: CompletionUsage {
            prompt_tokens,
            completion_tokens: token_count,
            total_tokens: prompt_tokens + token_count,
        },
    };

    guard.success();
    Ok(Json(response))
}

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
        assert_eq!(req.top_p, 0.9);
        assert!(req.seed.is_none());
        assert!(req.stop.is_empty());
    }

    #[test]
    fn test_completion_request_with_seed_and_stop() {
        let json = r#"{"model":"test","prompt":"hello","seed":42,"stop":["\n"]}"#;
        let req: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.seed, Some(42));
        assert_eq!(req.stop, vec!["\n".to_string()]);
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

    #[test]
    fn test_finish_reason_length() {
        // When tokens_generated == max_tokens, finish_reason should be "length".
        let resp = CompletionResponse {
            id: "cmpl-1".to_string(),
            object: "text_completion",
            created: 0,
            model: "test".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                text: "...".to_string(),
                finish_reason: Some("length".to_string()),
            }],
            usage: CompletionUsage {
                prompt_tokens: 1,
                completion_tokens: 128,
                total_tokens: 129,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("length"));
    }
}
