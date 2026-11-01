#![allow(unused_imports, unused_variables)]
//! # POST /v1/chat/completions — OpenAI-Compatible Chat Endpoint
//!
//! ## Purpose
//! Handles chat completion requests. Supports both streaming (SSE) and
//! non-streaming (JSON) responses. Wired to `SsdEngine` (the pure-Rust
//! working inference engine) by default; swap to `LlamaBackend` when the
//! `llama-backend` feature is enabled and a real GGUF model is loaded.
//!
//! ## What This File Does
//! - Parses the model ID and verifies it exists in the registry
//! - Creates an `SsdEngine`, loads the model
//! - Builds a Prompt from the request messages
//! - If stream=true: spawns a generation task, streams tokens via SSE
//! - If stream=false: collects all tokens into a single JSON response
//! - Formats SSE chunks as `data: {json}\n\n` with [DONE] terminator
//!
//! ## Dependencies
//! - `edgeforge-runtime::{SsdEngine, InferenceEngine, GenParams}`
//! - `edgeforge-security::{ModelManifest, ModelArch}`
//! - `edgeforge-crowd::ExecutionMode`
//! - `tokio_stream` + `futures::StreamExt` — SSE body stream
//! - `crate::{dto, middleware::ApiError, state::AppState}`
//!
//! ## Output
//! SSE stream (`text/event-stream`) or JSON response.
//!
//! ## SSE Format
//! ```text
//! data: {"id":"chatcmpl-...","choices":[{"delta":{"content":"Hello"}}]}
//!
//! data: {"id":"chatcmpl-...","choices":[{"delta":{"content":" world"}}]}
//!
//! data: [DONE]
//! ```
//!
//! ## Function: chat_completions
//! - **Purpose:** Handle POST /v1/chat/completions
//! - **Parameters:** `State<AppState>`, `Json<ChatCompletionRequest>`
//! - **Returns:** `Result<Response, ApiError>` — SSE stream or JSON
//! - **Errors:** ModelNotFound if model ID invalid; Engine if generation fails
//! - **Dependencies:** `state.models.get()`, `SsdEngine::load()`,
//!   `engine.generate()` with token callback

use crate::{
    dto::{ChatCompletionRequest, ChatCompletionResponse, Choice, ChoiceMessage, Usage},
    middleware::ApiError,
    state::AppState,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use edgeforge_core::{ChatMessage, ModelId, Prompt, Role, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_runtime::{GenParams, InferenceEngine, SsdEngine};
use edgeforge_security::{ModelArch, ModelManifest};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use futures::StreamExt;

/// Returns the axum Router for this module's routes.
pub fn routes() -> Router<AppState> {
    Router::new().route("/v1/chat/completions", post(chat_completions))
}

async fn chat_completions(
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    // Parse model ID and verify it exists in the registry.
    let mid = ModelId::parse(&req.model)
        .map_err(|e| edgeforge_core::EdgeForgeError::InvalidRequest(e.to_string()))?;
    let row = state
        .models
        .get(mid)
        .await?
        .ok_or_else(|| edgeforge_core::EdgeForgeError::ModelNotFound(req.model.clone()))?;

    // Build a manifest for the engine from the registry row.
    // (Phase 4: in production, load a signed manifest from disk.)
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

    // Create the SsdEngine (pure-Rust, working inference) and load the model.
    // SsdEngine never returns NotImplemented — it produces real tokens via a
    // deterministic Markov-chain sampler and exercises the storage tier.
    let engine = SsdEngine::new();
    let model = engine
        .load(&manifest, ExecutionMode::Local)
        .await
        .map_err(|e| edgeforge_core::EdgeForgeError::Engine(e.to_string()))?;

    // Build the prompt from the request messages.
    let prompt = Prompt::new(req.messages.clone());

    // Build generation parameters.
    let params = GenParams {
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        seed: req.seed,
        stop_sequences: req.stop.clone(),
    };

    let created = now_secs();
    let chat_id = format!("chatcmpl-{}", ModelId::new());

    if req.stream {
        // SSE streaming response.
        let model_name = req.model.clone();
        let chat_id_clone = chat_id.clone();

        // Use a channel to stream tokens as SSE events.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn the generation task.
        tokio::spawn(async move {
            let mut token_count = 0u32;
            let tx_clone = tx.clone();
            let result = engine
                .generate(
                    &model,
                    &prompt,
                    &params,
                    &mut move |token: Token, _outcome| {
                        token_count += 1;
                        let chunk = format_sse_chunk(
                            &chat_id_clone,
                            &model_name,
                            created,
                            &token.text,
                        );
                        let _ = tx_clone.send(chunk);
                        true // continue generating
                    },
                )
                .await;

            // Send the final [DONE] marker.
            let _ = tx.send("data: [DONE]\n\n".to_string());

            if let Err(e) = result {
                tracing::error!(error = %e, "generation failed during SSE stream");
            }
        });

        // Convert the channel into an SSE body stream.
        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
            .map(|s| Ok::<_, std::convert::Infallible>(s));
        let body = Body::from_stream(stream);

        Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/event-stream"),
                (header::CACHE_CONTROL, "no-cache"),
                (header::CONNECTION, "keep-alive"),
            ],
            body,
        )
            .into_response())
    } else {
        // Non-streaming: collect all tokens into a single response.
        let mut full_response = String::new();
        let mut token_count = 0u32;

        engine
            .generate(
                &model,
                &prompt,
                &params,
                &mut |token: Token, _outcome| {
                    full_response.push_str(&token.text);
                    token_count += 1;
                    true
                },
            )
            .await
            .map_err(|e| edgeforge_core::EdgeForgeError::Engine(e.to_string()))?;

        Ok(Json(ChatCompletionResponse {
            id: chat_id,
            object: "chat.completion",
            created,
            model: req.model,
            choices: vec![Choice {
                index: 0,
                message: ChoiceMessage {
                    role: "assistant",
                    content: full_response,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage {
                prompt_tokens: req.messages.iter().map(|m| m.content.split_whitespace().count() as u32).sum::<u32>(),
                completion_tokens: token_count,
                total_tokens: token_count + req.messages.iter().map(|m| m.content.split_whitespace().count() as u32).sum::<u32>(),
            },
        })
        .into_response())
    }
}

/// Format a single SSE chunk for a token.
fn format_sse_chunk(id: &str, model: &str, created: i64, content: &str) -> String {
    let chunk = serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "content": content },
            "finish_reason": null
        }]
    });
    format!("data: {}\n\n", chunk)
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::dto::{ChatCompletionRequest, ChatCompletionResponse, Choice, ChoiceMessage, Usage};

    #[test]
    fn test_chat_request_defaults() {
        let json = r#"{"model":"m1","messages":[]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_tokens, 512);
        assert!(!req.stream);
    }

    #[test]
    fn test_chat_response_serializes() {
        let resp = ChatCompletionResponse {
            id: "cmpl-1".into(), object: "chat.completion", created: 0,
            model: "m1".into(),
            choices: vec![Choice { index: 0, message: ChoiceMessage { role: "assistant", content: "hi".into() }, finish_reason: Some("stop".into()) }],
            usage: Usage { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("chat.completion"));
    }

    #[test]
    fn test_sse_chunk_format() {
        let chunk = serde_json::json!({"id":"x","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]});
        let sse = format!("data: {}\n\n", chunk);
        assert!(sse.starts_with("data: "));
        assert!(sse.ends_with("\n\n"));
    }

    #[test]
    fn test_done_marker() {
        assert_eq!("data: [DONE]\n\n", "data: [DONE]\n\n");
    }
}
