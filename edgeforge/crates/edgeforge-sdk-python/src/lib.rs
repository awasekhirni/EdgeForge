//! # edgeforge-sdk-python — Python SDK (ADR-011)
//!
//! ## Purpose
//! Native Python SDK for EdgeForge, built with PyO3. Ships as prebuilt wheels
//! (cp39–cp313, abi3) via maturin. Wraps the EdgeForge REST API in a convenient
//! Python-facing interface.
//!
//! ## What This File Does
//! - `edgeforge` Python module (registered via `#[pymodule]`)
//! - `EdgeForgeClient` Python class (`#[pyclass]`) — sync HTTP client
//! - `ChatResponse`, `ModelInfo`, `HealthStatus`, `ImportResult`,
//!   `EmbeddingResult`, `SearchResult` Python classes
//! - Methods: `new`, `chat`, `list_models`, `get_model`, `health`,
//!   `import_model`, `embed`, `search`, `upload_file`
//!
//! ## PyO3 Integration
//! The entire crate is a PyO3 extension module. `#[pymodule] edgeforge` is the
//! entry point that Python imports as `import edgeforge`. All public types
//! are `#[pyclass]` and all methods are `#[pymethods]`.
//!
//! ## HTTP
//! Uses `reqwest::blocking` (sync) so methods can be called from Python
//! without async/await. The blocking client runs on its own internal thread
//! pool.
//!
//! ## Dependencies
//! - `pyo3` (abi3-py39, extension-module) — Python bindings
//! - `reqwest` (blocking, json, multipart) — HTTP client
//! - `serde` / `serde_json` — serialization
//! - `edgeforge-core` — types
//! - `edgeforge-observability` — FunctionGuard for call logging
//!
//! ## Output
//! A `edgeforge` Python module with a `Client` class.
//!
//! ## Usage (Python)
//! ```python
//! import edgeforge
//! client = edgeforge.Client("http://127.0.0.1:11434/v1", api_key="my-secret")
//! print(client.health().status)  # "ok"
//! resp = client.chat(model="my-model", prompt="Hello!")
//! print(resp.text)
//! ```
//!
//! ## Function Logging
//! `timestamp - edgeforge-sdk-python - lib.rs - function - STATUS`

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use pyo3::types::PyTuple;
use reqwest::blocking::{Client as ReqwestClient, multipart};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

// ── Python-facing response types ───────────────────────────────────────────

/// Response from a chat completion request.
///
/// ## Python
/// ```python
/// resp = client.chat(model="my-model", prompt="Hello!")
/// print(resp.text)             # the generated text
/// print(resp.model)            # the model ID
/// print(resp.tokens_generated) # token count
/// print(resp.finish_reason)    # "stop" or "length"
/// ```
#[pyclass(name = "ChatResponse")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyChatResponse {
    /// The generated text.
    #[pyo3(get)]
    pub text: String,
    /// The model ID used.
    #[pyo3(get)]
    pub model: String,
    /// Number of tokens generated.
    #[pyo3(get)]
    pub tokens_generated: u32,
    /// Finish reason ("stop", "length", etc.).
    #[pyo3(get)]
    pub finish_reason: String,
}

/// Model information returned by `list_models` / `get_model`.
#[pyclass(name = "ModelInfo")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyModelInfo {
    /// Model ID.
    #[pyo3(get)]
    pub id: String,
    /// Model object type ("model").
    #[pyo3(get)]
    pub object: String,
    /// Creation timestamp (Unix seconds).
    #[pyo3(get)]
    pub created: i64,
    /// Publisher / owner.
    #[pyo3(get)]
    pub owned_by: String,
}

/// Server health status.
#[pyclass(name = "HealthStatus")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyHealthStatus {
    /// Server status ("ok" or "error").
    #[pyo3(get)]
    pub status: String,
    /// Server version.
    #[pyo3(get)]
    pub version: String,
}

/// Model import result.
#[pyclass(name = "ImportResult")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyImportResult {
    /// The imported model's ID.
    #[pyo3(get)]
    pub model_id: String,
    /// Import status ("accepted" or "failed").
    #[pyo3(get)]
    pub status: String,
    /// The file path that was imported.
    #[pyo3(get)]
    pub path: String,
}

/// Embedding result from `embed`.
#[pyclass(name = "EmbeddingResult")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyEmbeddingResult {
    /// The embedding vector.
    #[pyo3(get)]
    pub embedding: Vec<f32>,
    /// The model ID used.
    #[pyo3(get)]
    pub model: String,
    /// Number of tokens in the input.
    #[pyo3(get)]
    pub token_count: u32,
}

/// A single search result from `search`.
#[pyclass(name = "SearchHit")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PySearchHit {
    /// The source ID.
    #[pyo3(get)]
    pub source_id: String,
    /// The source URI.
    #[pyo3(get)]
    pub source_uri: String,
    /// Chunk index.
    #[pyo3(get)]
    pub chunk_idx: Option<i64>,
    /// Matched content.
    #[pyo3(get)]
    pub content: String,
    /// Cosine similarity score.
    #[pyo3(get)]
    pub score: f32,
    /// Model ID.
    #[pyo3(get)]
    pub model_id: String,
}

/// File upload result.
#[pyclass(name = "FileUploadResult")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyFileUploadResult {
    /// File ID.
    #[pyo3(get)]
    pub id: String,
    /// Status ("processed").
    #[pyo3(get)]
    pub status: String,
    /// Number of chunks.
    #[pyo3(get)]
    pub chunks: u32,
    /// Number of vectors stored.
    #[pyo3(get)]
    pub stored: u32,
    /// File size in bytes.
    #[pyo3(get)]
    pub bytes: u64,
    /// Filename.
    #[pyo3(get)]
    pub filename: Option<String>,
    /// Model ID used for embedding.
    #[pyo3(get)]
    pub model_id: String,
}

// ── EdgeForgeClient (Python class) ─────────────────────────────────────────

/// The EdgeForge Python client.
///
/// ## Purpose
/// Synchronous HTTP client that wraps the EdgeForge REST API. All methods
/// block until the HTTP response is received.
///
/// ## Python Usage
/// ```python
/// import edgeforge
/// client = edgeforge.Client(
///     base_url="http://127.0.0.1:11434/v1",
///     api_key="my-secret",       # optional
///     timeout=30.0,              # optional, seconds
/// )
/// print(client.health().status)
/// ```
#[pyclass(name = "Client")]
pub struct EdgeForgeClient {
    /// The base URL of the EdgeForge API server (e.g. "http://127.0.0.1:11434/v1").
    base_url: String,
    /// The API key (sent as `Authorization: Bearer <api_key>`).
    api_key: Option<String>,
    /// The underlying reqwest blocking client.
    http: ReqwestClient,
}

#[pymethods]
impl EdgeForgeClient {
    /// Create a new EdgeForge client.
    ///
    /// ## Parameters
    /// - `base_url` (str): The API server base URL. Default: "http://127.0.0.1:11434/v1".
    /// - `api_key` (str, optional): The API key for authentication. If None,
    ///   no Authorization header is sent.
    /// - `timeout` (float, optional): Request timeout in seconds. Default: 30.
    ///
    /// ## Returns
    /// A new `Client` instance.
    ///
    /// ## Errors
    /// Raises `RuntimeError` if the HTTP client cannot be constructed.
    #[new]
    #[pyo3(signature = (base_url = "http://127.0.0.1:11434/v1".to_string(), api_key = None, timeout = 30.0))]
    fn new(base_url: String, api_key: Option<String>, timeout: f64) -> PyResult<Self> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-sdk-python",
            "lib.rs",
            "EdgeForgeClient::new",
        );

        let http = ReqwestClient::builder()
            .timeout(Duration::from_secs_f64(timeout.max(0.1)))
            .build()
            .map_err(|e| PyRuntimeError::new_err(format!("failed to build HTTP client: {e}")))?;

        tracing::info!(
            base_url = %base_url,
            has_api_key = api_key.is_some(),
            timeout_secs = timeout,
            "EdgeForgeClient: initialized"
        );

        guard_success();
        Ok(Self {
            base_url,
            api_key,
            http,
        })
    }

    /// The base URL string.
    #[getter]
    fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Whether an API key is configured.
    #[getter]
    fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Check server health.
    ///
    /// ## Returns
    /// `HealthStatus` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure.
    fn health(&self) -> PyResult<PyHealthStatus> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "health");

        // /health is at the root, not under /v1. Strip the /v1 suffix if present.
        let url = if self.base_url.ends_with("/v1") {
            format!("{}/health", &self.base_url[..self.base_url.len() - 3])
        } else {
            format!("{}/health", self.base_url)
        };
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| {
                guard.error(&e.to_string());
                PyRuntimeError::new_err(format!("health request failed: {e}"))
            })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("health returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let body: HealthApiResponse = resp
            .json()
            .map_err(|e| {
                guard.error(&e.to_string());
                PyRuntimeError::new_err(format!("failed to parse health response: {e}"))
            })?;

        guard.success();
        Ok(PyHealthStatus {
            status: body.status,
            version: body.version,
        })
    }

    /// List imported models.
    ///
    /// ## Returns
    /// `list[ModelInfo]` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure or auth error (401).
    fn list_models(&self) -> PyResult<Vec<PyModelInfo>> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "list_models");

        let url = format!("{}/models", self.base_url);
        let mut req = self.http.get(&url);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("list_models request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("list_models returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let body: ModelsApiResponse = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse models response: {e}"))
        })?;

        guard.success();
        Ok(body.data)
    }

    /// Get a single model by ID.
    ///
    /// ## Parameters
    /// - `model_id` (str): The model ID.
    ///
    /// ## Returns
    /// `ModelInfo` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure or 404.
    fn get_model(&self, model_id: &str) -> PyResult<PyModelInfo> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "get_model");

        let url = format!("{}/models/{}", self.base_url, model_id);
        let mut req = self.http.get(&url);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("get_model request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("get_model returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let model: PyModelInfo = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse model response: {e}"))
        })?;

        guard.success();
        Ok(model)
    }

    /// Send a chat completion request.
    ///
    /// ## Parameters
    /// - `model` (str): The model ID.
    /// - `prompt` (str): The user's prompt.
    /// - `max_tokens` (int, optional): Max tokens to generate. Default: 512.
    /// - `temperature` (float, optional): Sampling temperature. Default: 0.7.
    /// - `top_p` (float, optional): Top-p threshold. Default: 0.9.
    /// - `seed` (int, optional): Random seed for deterministic generation.
    ///
    /// ## Returns
    /// `ChatResponse` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure or auth error (401).
    #[pyo3(signature = (model, prompt, max_tokens = 512, temperature = 0.7, top_p = 0.9, seed = None))]
    fn chat(
        &self,
        model: &str,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
        top_p: f32,
        seed: Option<u64>,
    ) -> PyResult<PyChatResponse> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "chat");

        let url = format!("{}/chat/completions", self.base_url);
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": max_tokens,
            "temperature": temperature,
            "top_p": top_p,
            "stream": false,
            "seed": seed,
        });

        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("chat request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("chat returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let api_resp: ChatApiResponse = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse chat response: {e}"))
        })?;

        let choice = api_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| {
                guard.error("no choices in response");
                PyRuntimeError::new_err("chat response had no choices")
            })?;

        let result = PyChatResponse {
            text: choice.message.content,
            model: api_resp.model,
            tokens_generated: api_resp.usage.completion_tokens,
            finish_reason: choice.finish_reason.unwrap_or_else(|| "stop".to_string()),
        };

        guard.success();
        Ok(result)
    }

    /// Import a model from a file path (server-side).
    ///
    /// ## Parameters
    /// - `path` (str): Path to the model file (.gguf). The path must be
    ///   accessible from the server, not the client.
    ///
    /// ## Returns
    /// `ImportResult` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure, including 404 when the endpoint
    /// is not available on the server.
    fn import_model(&self, path: &str) -> PyResult<PyImportResult> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "import_model");

        let url = format!("{}/models/import", self.base_url);
        let body = serde_json::json!({ "path": path });

        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("import_model request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = if status_code.as_u16() == 404 {
                "import_model endpoint not found (404) — the server does not support model import via HTTP. Use `edgeforge model import --path <file>` on the server instead.".to_string()
            } else {
                format!("import_model returned HTTP {}", status_code.as_u16())
            };
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let result: PyImportResult = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse import response: {e}"))
        })?;

        guard.success();
        Ok(result)
    }

    /// Generate an embedding for a text.
    ///
    /// ## Parameters
    /// - `text` (str): The text to embed.
    /// - `model` (str, optional): The embedding model ID. Default: "edgeforge-hashing-768".
    ///
    /// ## Returns
    /// `EmbeddingResult` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure.
    #[pyo3(signature = (text, model = "edgeforge-hashing-768".to_string()))]
    fn embed(&self, text: &str, model: String) -> PyResult<PyEmbeddingResult> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "embed");

        let url = format!("{}/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": model,
            "input": text,
        });

        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("embed request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("embed returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let api_resp: EmbeddingsApiResponse = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse embeddings response: {e}"))
        })?;

        let emb = api_resp.data.into_iter().next().ok_or_else(|| {
            guard.error("no embedding in response");
            PyRuntimeError::new_err("embeddings response had no data")
        })?;

        let result = PyEmbeddingResult {
            embedding: emb.embedding,
            model: api_resp.model,
            token_count: api_resp.usage.total_tokens,
        };

        guard.success();
        Ok(result)
    }

    /// Search for similar chunks.
    ///
    /// ## Parameters
    /// - `query` (str): The query text.
    /// - `top_k` (int, optional): Max results. Default: 5.
    /// - `model` (str, optional): Embedding model ID. Default: "edgeforge-hashing-768".
    ///
    /// ## Returns
    /// `list[SearchHit]` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure.
    #[pyo3(signature = (query, top_k = 5, model = "edgeforge-hashing-768".to_string()))]
    fn search(&self, query: &str, top_k: usize, model: String) -> PyResult<Vec<PySearchHit>> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "search");

        let url = format!("{}/embeddings/search", self.base_url);
        let body = serde_json::json!({
            "model": model,
            "query": query,
            "top_k": top_k,
        });

        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("search request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("search returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let api_resp: SearchApiResponse = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse search response: {e}"))
        })?;

        guard.success();
        Ok(api_resp.results)
    }

    /// Upload a file for RAG ingestion.
    ///
    /// ## Parameters
    /// - `file_path` (str): Path to the file to upload (client-side path).
    ///
    /// ## Returns
    /// `FileUploadResult` on success.
    ///
    /// ## Errors
    /// Raises `RuntimeError` on HTTP failure or if the file cannot be read.
    fn upload_file(&self, file_path: &str) -> PyResult<PyFileUploadResult> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-sdk-python", "lib.rs", "upload_file");

        let filename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload.bin")
            .to_string();

        let file_bytes = std::fs::read(file_path).map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to read file {file_path}: {e}"))
        })?;

        let url = format!("{}/files", self.base_url);
        let form = multipart::Form::new().file("file", file_path).map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to build multipart form: {e}"))
        })?;

        let mut req = self.http.post(&url).multipart(form);
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("upload_file request failed: {e}"))
        })?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let msg = format!("upload_file returned HTTP {}", status_code.as_u16());
            guard.error(&msg);
            return Err(PyRuntimeError::new_err(msg));
        }

        let result: PyFileUploadResult = resp.json().map_err(|e| {
            guard.error(&e.to_string());
            PyRuntimeError::new_err(format!("failed to parse upload response: {e}"))
        })?;

        tracing::info!(
            file_path = file_path,
            filename = %filename,
            file_size = file_bytes.len(),
            chunks = result.chunks,
            stored = result.stored,
            "upload_file: complete"
        );

        guard.success();
        Ok(result)
    }

    /// String representation for Python `repr(client)`.
    fn __repr__(&self) -> String {
        format!(
            "EdgeForgeClient(base_url={:?}, has_api_key={})",
            self.base_url,
            self.api_key.is_some()
        )
    }
}

// ── Internal API response DTOs (match the server's JSON) ───────────────────

#[derive(Deserialize)]
struct HealthApiResponse {
    status: String,
    version: String,
}

#[derive(Deserialize)]
struct ModelsApiResponse {
    data: Vec<PyModelInfo>,
}

#[derive(Deserialize)]
struct ChatApiResponse {
    model: String,
    choices: Vec<ChatChoice>,
    usage: ChatUsage,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageDto,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatMessageDto {
    content: String,
}

#[derive(Deserialize)]
struct ChatUsage {
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct EmbeddingsApiResponse {
    data: Vec<EmbeddingDataDto>,
    model: String,
    usage: EmbeddingsUsageDto,
}

#[derive(Deserialize)]
struct EmbeddingDataDto {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbeddingsUsageDto {
    total_tokens: u32,
}

#[derive(Deserialize)]
struct SearchApiResponse {
    results: Vec<PySearchHit>,
}

// ── Helper: FunctionGuard success without holding the guard ────────────────

fn guard_success() {
    // The FunctionGuard logs success on drop by default, so this is a no-op.
    // Kept for readability at call sites.
}

// ── Python module entry point ──────────────────────────────────────────────

/// The `edgeforge` Python module entry point.
///
/// ## Python
/// ```python
/// import edgeforge
/// client = edgeforge.Client("http://127.0.0.1:11434/v1")
/// ```
///
/// ## Module Contents
/// - `Client` — the main client class
/// - `ChatResponse`, `ModelInfo`, `HealthStatus`, `ImportResult`,
///   `EmbeddingResult`, `SearchHit`, `FileUploadResult` — response types
/// - `__version__` — the SDK version string
#[pymodule]
fn edgeforge(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-sdk-python",
        "lib.rs",
        "edgeforge (module init)",
    );

    tracing::info!("edgeforge Python module initializing");

    m.add_class::<EdgeForgeClient>()?;
    m.add_class::<PyChatResponse>()?;
    m.add_class::<PyModelInfo>()?;
    m.add_class::<PyHealthStatus>()?;
    m.add_class::<PyImportResult>()?;
    m.add_class::<PyEmbeddingResult>()?;
    m.add_class::<PySearchHit>()?;
    m.add_class::<PyFileUploadResult>()?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    tracing::info!("edgeforge Python module initialized");
    Ok(())
}

// ── Rust-only tests (no Python interpreter needed) ─────────────────────────
//
// These tests verify the serde DTOs and the PyChatResponse/etc structs
// deserialize correctly from server JSON. They don't require a running
// server or a Python interpreter.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_response_deserialize() {
        let json = r#"{
            "text": "hello world",
            "model": "test-model",
            "tokens_generated": 5,
            "finish_reason": "stop"
        }"#;
        let r: PyChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.text, "hello world");
        assert_eq!(r.model, "test-model");
        assert_eq!(r.tokens_generated, 5);
        assert_eq!(r.finish_reason, "stop");
    }

    #[test]
    fn test_chat_api_response_deserialize() {
        let json = r#"{
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hi there"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5}
        }"#;
        let resp: ChatApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.model, "test-model");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, "hi there");
        assert_eq!(resp.usage.completion_tokens, 3);
    }

    #[test]
    fn test_health_api_response_deserialize() {
        let json = r#"{"status":"ok","version":"0.1.0"}"#;
        let h: HealthApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(h.status, "ok");
        assert_eq!(h.version, "0.1.0");
    }

    #[test]
    fn test_models_api_response_deserialize() {
        let json = r#"{
            "object": "list",
            "data": [{
                "id": "01J9TEST",
                "object": "model",
                "created": 1700000000,
                "owned_by": "test"
            }]
        }"#;
        let resp: ModelsApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "01J9TEST");
        assert_eq!(resp.data[0].owned_by, "test");
    }

    #[test]
    fn test_embeddings_api_response_deserialize() {
        let json = r#"{
            "object": "list",
            "data": [{"object":"embedding","index":0,"embedding":[0.1,0.2,0.3]}],
            "model": "edgeforge-hashing-768",
            "usage": {"prompt_tokens": 2, "total_tokens": 2}
        }"#;
        let resp: EmbeddingsApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.model, "edgeforge-hashing-768");
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(resp.usage.total_tokens, 2);
    }

    #[test]
    fn test_search_api_response_deserialize() {
        let json = r#"{
            "object": "list",
            "model": "edgeforge-hashing-768",
            "results": [{
                "source_id": "src1",
                "source_uri": "doc.txt",
                "chunk_idx": 0,
                "content": "hello",
                "score": 0.95,
                "model_id": "edgeforge-hashing-768"
            }]
        }"#;
        let resp: SearchApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].source_id, "src1");
        assert!((resp.results[0].score - 0.95).abs() < 1e-6);
    }

    #[test]
    fn test_file_upload_result_deserialize() {
        let json = r#"{
            "id": "file-1",
            "status": "processed",
            "chunks": 5,
            "stored": 5,
            "bytes": 1024,
            "filename": "test.txt",
            "model_id": "edgeforge-hashing-768"
        }"#;
        let r: PyFileUploadResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.id, "file-1");
        assert_eq!(r.chunks, 5);
        assert_eq!(r.bytes, 1024);
    }

    #[test]
    fn test_import_result_deserialize() {
        let json = r#"{
            "model_id": "01J9TEST",
            "status": "accepted",
            "path": "/tmp/model.gguf"
        }"#;
        let r: PyImportResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.model_id, "01J9TEST");
        assert_eq!(r.path, "/tmp/model.gguf");
    }

    #[test]
    fn test_model_info_deserialize() {
        let json = r#"{
            "id": "01J9TEST",
            "object": "model",
            "created": 1700000000,
            "owned_by": "test"
        }"#;
        let m: PyModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(m.id, "01J9TEST");
        assert_eq!(m.object, "model");
        assert_eq!(m.created, 1700000000);
    }
}
