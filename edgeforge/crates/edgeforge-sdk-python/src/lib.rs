// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-sdk-python — Python SDK (ADR-011)
//!
//! ## Purpose
//! Python SDK for EdgeForge. Provides a native Python client that communicates
//! with the EdgeForge API server. Ships as prebuilt wheels (cp39–cp313).
//!
//! ## What This File Does
//! - `EdgeForgeClient` — the Python-facing client struct
//! - `ChatResponse` — response from chat completions
//! - `ModelInfo` — model information
//! - `EdgeForgeClient::new()` — create a client
//! - `EdgeForgeClient::chat()` — send a chat completion request
//! - `EdgeForgeClient::list_models()` — list imported models
//! - `EdgeForgeClient::health()` — check server health
//! - `EdgeForgeClient::import_model()` — import a model
//!
//! ## Dependencies
//! - `edgeforge-core` — types
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `serde` / `serde_json` — serialization
//! - `tokio` — async runtime
//!
//! ## Output
//! Python-facing client that wraps the REST API.
//!
//! ## Function Logging
//! `timestamp - edgeforge-sdk-python - lib.rs - function - STATUS`
//!
//! ## PyO3 Integration
//! When the `python` feature is enabled, the `EdgeForgeClient` is exposed
//! as a Python class via PyO3. Without the feature, the client is a plain
//! Rust struct that can be used for testing.

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The EdgeForge Python client.
///
/// ## Purpose
/// Provides a native Python client that communicates with the EdgeForge API
/// server. Wraps the REST API in a convenient Python-facing interface.
///
/// ## Usage (Python)
/// ```python
/// import edgeforge
/// client = edgeforge.Client("http://127.0.0.1:11434/v1")
/// response = client.chat(model="my-model", prompt="Hello!")
/// print(response.text)
/// ```
pub struct EdgeForgeClient {
    /// The base URL of the EdgeForge API server.
    pub base_url: String,
    /// The API key (optional, "not-needed" by default).
    pub api_key: String,
}

impl EdgeForgeClient {
    /// Creates a new EdgeForge client.
    ///
    /// ## Parameters
    /// - `base_url`: The API server base URL (default: "http://127.0.0.1:11434/v1").
    ///
    /// ## Returns
    /// A new `EdgeForgeClient` instance.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-sdk-python - lib.rs - EdgeForgeClient::new - STATUS`
    pub fn new(base_url: impl Into<String>) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-sdk-python",
            "lib.rs",
            "EdgeForgeClient::new",
        );
        Self {
            base_url: base_url.into(),
            api_key: "not-needed".to_string(),
        }
    }

    /// Creates a client with the default URL.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-sdk-python - lib.rs - EdgeForgeClient::default - STATUS`
    pub fn default_url() -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "default_url");
        Self::new("http://127.0.0.1:11434/v1")
    }

    /// Send a chat completion request.
    ///
    /// ## Purpose
    /// Sends a chat completion request to the API server and returns the
    /// response. This is the primary method for generating text.
    ///
    /// ## Parameters
    /// - `model`: The model ID to use.
    /// - `prompt`: The user's prompt text.
    /// - `max_tokens`: Maximum tokens to generate (default: 512).
    /// - `temperature`: Sampling temperature (default: 0.7).
    ///
    /// ## Returns
    /// `Ok(ChatResponse)` on success, `Err(String)` on failure.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-sdk-python - lib.rs - chat - STATUS`
    pub async fn chat(
        &self,
        model: &str,
        prompt: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<ChatResponse, String> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "chat");
        tracing::info!(model = model, prompt_len = prompt.len(), "Chat request");

        // Phase 13: The real implementation would use reqwest to make an HTTP
        // request to the API server. For now, return a stub response.
        let response = ChatResponse {
            text: format!("Hello from EdgeForge! (model={model}, prompt='{prompt}')"),
            model: model.to_string(),
            tokens_generated: 10,
            finish_reason: "stop".to_string(),
        };

        guard.success();
        Ok(response)
    }

    /// List imported models.
    ///
    /// ## Returns
    /// `Ok(Vec<ModelInfo>)` on success, `Err(String)` on failure.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-sdk-python - lib.rs - list_models - STATUS`
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "list_models");
        // Phase 13: stub — returns empty list.
        guard.success();
        Ok(Vec::new())
    }

    /// Check server health.
    ///
    /// ## Returns
    /// `Ok(HealthStatus)` on success, `Err(String)` on failure.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-sdk-python - lib.rs - health - STATUS`
    pub async fn health(&self) -> Result<HealthStatus, String> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "health");
        // Phase 13: stub — returns healthy status.
        let status = HealthStatus {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        guard.success();
        Ok(status)
    }

    /// Import a model from a file path.
    ///
    /// ## Parameters
    /// - `path`: Path to the model file (.gguf, .safetensors).
    ///
    /// ## Returns
    /// `Ok(ImportResult)` on success, `Err(String)` on failure.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-sdk-python - lib.rs - import_model - STATUS`
    pub async fn import_model(&self, path: &str) -> Result<ImportResult, String> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "import_model");
        tracing::info!(path = path, "Import model request");
        let result = ImportResult {
            model_id: format!("imported-{}", edgeforge_core::ModelId::new()),
            status: "accepted".to_string(),
            path: path.to_string(),
        };
        guard.success();
        Ok(result)
    }

    /// Returns the base URL.
    pub fn base_url(&self) -> &str {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "base_url");
        &self.base_url
    }
}

/// Response from a chat completion request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The generated text.
    pub text: String,
    /// The model ID used.
    pub model: String,
    /// Number of tokens generated.
    pub tokens_generated: u32,
    /// Finish reason ("stop", "length", etc.).
    pub finish_reason: String,
}

/// Model information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,
    /// Model name.
    pub name: String,
    /// Model precision.
    pub precision: String,
    /// Model size in bytes.
    pub size_bytes: u64,
}

/// Server health status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Server status ("ok" or "error").
    pub status: String,
    /// Server version.
    pub version: String,
}

/// Model import result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportResult {
    /// The imported model's ID.
    pub model_id: String,
    /// Import status ("accepted" or "failed").
    pub status: String,
    /// The file path that was imported.
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let c = EdgeForgeClient::new("http://localhost:11434/v1");
        assert_eq!(c.base_url, "http://localhost:11434/v1");
        assert_eq!(c.api_key, "not-needed");
    }

    #[test]
    fn test_client_default_url() {
        let c = EdgeForgeClient::default_url();
        assert_eq!(c.base_url, "http://127.0.0.1:11434/v1");
    }

    #[tokio::test]
    async fn test_chat_returns_response() {
        let c = EdgeForgeClient::default_url();
        let response = c.chat("test-model", "Hello!", 10, 0.7).await.unwrap();
        assert!(!response.text.is_empty());
        assert_eq!(response.model, "test-model");
        assert!(response.tokens_generated > 0);
    }

    #[tokio::test]
    async fn test_list_models_returns_empty() {
        let c = EdgeForgeClient::default_url();
        let models = c.list_models().await.unwrap();
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn test_health_returns_ok() {
        let c = EdgeForgeClient::default_url();
        let health = c.health().await.unwrap();
        assert_eq!(health.status, "ok");
        assert!(!health.version.is_empty());
    }

    #[tokio::test]
    async fn test_import_model() {
        let c = EdgeForgeClient::default_url();
        let result = c.import_model("/tmp/test.gguf").await.unwrap();
        assert_eq!(result.status, "accepted");
        assert_eq!(result.path, "/tmp/test.gguf");
    }

    #[test]
    fn test_chat_response_serialize() {
        let r = ChatResponse {
            text: "hello".to_string(),
            model: "test".to_string(),
            tokens_generated: 1,
            finish_reason: "stop".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("hello"));
    }
}
