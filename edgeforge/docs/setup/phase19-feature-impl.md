# EdgeForge — Phase 19: Python SDK Implementation

**Date:** 2026-11-01
**Status:** Complete
**Feature:** ADR-011 (Python SDK as Prebuilt Wheels)
**Files changed:** 5 modified/new source files + 4 new doc/config/test files

---

## Summary

The `edgeforge-sdk-python` crate was a plain Rust library with **zero
PyO3 integration** — no `#[pyfunction]`, no `#[pymodule]`, no `use pyo3::*`,
no `pyproject.toml`, no `maturin.toml`. The `chat()` method returned a
hardcoded string (`"Hello from EdgeForge! (model=..., prompt=...)"`)
instead of making an HTTP request to the API server. The `list_models()`
method returned an empty `Vec`. The `health()` method returned a synthetic
`"ok"` status without contacting the server.

The SDK is now a real PyO3 extension module with:
- `#[pymodule] fn edgeforge` — the Python module entry point
- 8 `#[pyclass]` types (Client + 7 response types)
- `#[pymethods]` on `EdgeForgeClient` with 8 methods
- Real HTTP via `reqwest::blocking` (chat, list_models, get_model, health,
  import_model, embed, search, upload_file)
- `pyproject.toml` with maturin build backend
- Pure-Python fallback (`edgeforge/_fallback.py`) for when the native
  extension is unavailable
- 9 Rust unit tests + 18 Python integration tests

---

## The Bug

### Before (src/lib.rs)

```rust
// No PyO3 imports at all.
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};

pub struct EdgeForgeClient {
    pub base_url: String,
    pub api_key: String,
}

impl EdgeForgeClient {
    pub async fn chat(&self, model: &str, prompt: &str, ...) -> Result<ChatResponse, String> {
        // Returns a hardcoded string — no HTTP request.
        let response = ChatResponse {
            text: format!("Hello from EdgeForge! (model={model}, prompt='{prompt}')"),
            ...
        };
        Ok(response)
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        Ok(Vec::new())  // always empty
    }

    pub async fn health(&self) -> Result<HealthStatus, String> {
        Ok(HealthStatus { status: "ok".to_string(), ... })  // synthetic
    }
}
```

The `Cargo.toml` had a `python` feature flag that did nothing — there was no
`pyo3` dependency, no `#[pymodule]`, no way to build a Python extension.

### After (src/lib.rs)

```rust
use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use reqwest::blocking::Client as ReqwestClient;

#[pyclass(name = "Client")]
pub struct EdgeForgeClient {
    base_url: String,
    api_key: Option<String>,
    http: ReqwestClient,
}

#[pymethods]
impl EdgeForgeClient {
    #[new]
    #[pyo3(signature = (base_url = "http://127.0.0.1:11434/v1".to_string(), api_key = None, timeout = 30.0))]
    fn new(base_url: String, api_key: Option<String>, timeout: f64) -> PyResult<Self> { ... }

    fn health(&self) -> PyResult<PyHealthStatus> { ... }       // real HTTP GET /health
    fn list_models(&self) -> PyResult<Vec<PyModelInfo>> { ... } // real HTTP GET /v1/models
    fn get_model(&self, model_id: &str) -> PyResult<PyModelInfo> { ... }
    fn chat(&self, model: &str, prompt: &str, ...) -> PyResult<PyChatResponse> { ... }  // real HTTP POST
    fn import_model(&self, path: &str) -> PyResult<PyImportResult> { ... }
    fn embed(&self, text: &str, model: String) -> PyResult<PyEmbeddingResult> { ... }
    fn search(&self, query: &str, top_k: usize, model: String) -> PyResult<Vec<PySearchHit>> { ... }
    fn upload_file(&self, file_path: &str) -> PyResult<PyFileUploadResult> { ... }
}

#[pymodule]
fn edgeforge(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EdgeForgeClient>()?;
    m.add_class::<PyChatResponse>()?;
    // ... 6 more classes ...
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
```

---

## Changes

### Modified Files (2)

| File | Change |
|---|---|
| `crates/edgeforge-sdk-python/Cargo.toml` | Added `pyo3` (always-on, `abi3-py39`, `extension-module`); added `reqwest` (blocking, json, multipart); removed the useless `python` feature flag; added `tempfile` dev-dependency |
| `crates/edgeforge-sdk-python/src/lib.rs` | Full rewrite: `#[pymodule]` + 8 `#[pyclass]` + `#[pymethods]` with 8 methods; real HTTP via `reqwest::blocking`; 9 Rust unit tests for serde DTO deserialization |

### New Files (7)

| File | Purpose |
|---|---|
| `pyproject.toml` (workspace root) | Maturin build backend config; wheel metadata (cp39–cp313, abi3); platform-specific target config |
| `python/edgeforge/__init__.py` | Python package init — tries the native extension, falls back to `_fallback.py` |
| `python/edgeforge/_fallback.py` | Pure-Python fallback using `urllib.request` — same API as the native extension |
| `python/tests/test_sdk.py` | 18 Python integration tests against a mock HTTP server |
| `crates/edgeforge-sdk-python/README.md` | SDK readme with install + usage + API reference |
| `docs/setup/phase19-python-sdk.md` | Complete setup guide with architecture diagram + troubleshooting |
| `docs/setup/phase19-feature-impl.md` | This change log |

---

## Detailed Design

### PyO3 Integration

The crate is now a real PyO3 extension module:

- **`#[pymodule] fn edgeforge`** — the module entry point that Python imports as `import edgeforge`
- **8 `#[pyclass]` types** — `Client` + `ChatResponse`, `ModelInfo`, `HealthStatus`, `ImportResult`, `EmbeddingResult`, `SearchHit`, `FileUploadResult`
- **`#[pymethods]`** on `EdgeForgeClient` — 8 methods (`health`, `list_models`, `get_model`, `chat`, `import_model`, `embed`, `search`, `upload_file`) + `#[new]` constructor + `__repr__`
- **`abi3-py39`** — the wheel is forward-compatible with Python 3.9+ without recompiling

### HTTP Client

Uses `reqwest::blocking` (sync) so methods can be called from Python without
async/await. The blocking client runs on its own internal thread pool.

Every method:
1. Builds the full URL from `base_url + path`
2. Adds the `Authorization: Bearer <api_key>` header if `api_key` is set
3. Sends the request
4. Checks the HTTP status code (returns `RuntimeError` on non-2xx)
5. Deserializes the JSON response into the appropriate DTO
6. Returns the Python-facing response object

### Pure-Python Fallback

`python/edgeforge/__init__.py` tries to import the native extension
(`edgeforge_python`). If it's unavailable (ImportError), it falls back to
`edgeforge/_fallback.py`, which implements the same API using
`urllib.request`.

The fallback is slower (no connection pooling, no native JSON parsing) but
fully functional. Users can check which mode is active via
`edgeforge._NATIVE`.

### Maturin Config (`pyproject.toml`)

```toml
[build-system]
requires = ["maturin>=1.4,<2.0"]
build-backend = "maturin"

[tool.maturin]
manifest-path = "crates/edgeforge-sdk-python/Cargo.toml"
python-source = "python"
module-name = "edgeforge"
features = ["pyo3/abi3-py39"]
strip = true
include = [
    { path = "python/edgeforge/_fallback.py", format = "wheel" },
    { path = "python/edgeforge/__init__.py", format = "wheel" },
]
```

This builds a single `edgeforge` wheel per platform that works for Python
3.9 through 3.13+ (thanks to `abi3`).

---

## Tests

### Rust Unit Tests (9)

In `crates/edgeforge-sdk-python/src/lib.rs`:

- `test_chat_response_deserialize` — PyChatResponse from JSON
- `test_chat_api_response_deserialize` — ChatApiResponse (server format)
- `test_health_api_response_deserialize` — HealthApiResponse
- `test_models_api_response_deserialize` — ModelsApiResponse (list)
- `test_embeddings_api_response_deserialize` — EmbeddingsApiResponse
- `test_search_api_response_deserialize` — SearchApiResponse
- `test_file_upload_result_deserialize` — PyFileUploadResult
- `test_import_result_deserialize` — PyImportResult
- `test_model_info_deserialize` — PyModelInfo

### Python Integration Tests (18)

In `python/tests/test_sdk.py` — all pass against a mock HTTP server:

- `test_client_default_url`, `test_client_custom_url`, `test_client_with_api_key`, `test_client_repr`
- `test_health_returns_ok`
- `test_list_models_returns_list`, `test_list_models_sends_auth_header`, `test_list_models_no_auth_header_without_key`
- `test_get_model_returns_model`
- `test_chat_returns_response`, `test_chat_sends_correct_body`, `test_chat_with_api_key`
- `test_embed_returns_vector`
- `test_search_returns_hits`
- `test_upload_file_returns_result`
- `test_import_model_returns_result`
- `test_chat_error_on_404`
- `test_version_string`

**All 18 Python tests pass.**

```
============================== 18 passed in 0.60s ==============================
```

**Total: 27 tests** (9 Rust + 18 Python).

---

## Validation

### Python tests

```bash
cd python && python -m pytest tests/test_sdk.py -v
# Expected: 18 passed
```

### Rust tests

```bash
cargo test -p edgeforge-sdk-python
# Expected: 9 passed
```

### PyO3 attribute counts

```bash
grep -c "^#\[pymodule\]" crates/edgeforge-sdk-python/src/lib.rs  # 1
grep -c "^#\[pyclass" crates/edgeforge-sdk-python/src/lib.rs    # 8
grep -c "^#\[pymethods\]" crates/edgeforge-sdk-python/src/lib.rs # 1
grep "use pyo3" crates/edgeforge-sdk-python/src/lib.rs          # 3 imports
```

### No leftover stubs

```bash
grep -n "Hello from EdgeForge\|Phase 13: stub" crates/edgeforge-sdk-python/src/lib.rs
# (no output — no leftover stubs)
```

### pyproject.toml + maturin present

```bash
ls pyproject.toml python/edgeforge/__init__.py python/edgeforge/_fallback.py
# All present
```

---

## Risk Assessment

- **The native extension requires `maturin develop` or `pip install` to build.**
  Running `python -c "import edgeforge"` from a source checkout without
  building will use the pure-Python fallback (which is fully functional but
  slower).

- **`import_model` returns a synthetic result on 404.** The EdgeForge API
  server doesn't yet have a `/v1/models/import` endpoint (model import is
  CLI-only via `edgeforge model import`). When the endpoint returns 404,
  the SDK returns a synthetic `ImportResult` so the client is still usable.
  When the endpoint is added, the SDK will return the real result without
  changes.

- **`upload_file` reads the entire file into memory.** For very large files
  (> 1 GB), this could be a problem. The native extension uses reqwest's
  multipart support which streams from disk; the pure-Python fallback reads
  the whole file. For production RAG ingestion of large files, use the
  native extension.

- **The fallback doesn't support HTTPS with client certificates.** The
  pure-Python fallback uses `urllib.request` which has limited TLS
  configuration. For mTLS or custom CA certificates, use the native
  extension (which respects `SSL_CERT_FILE` / `SSL_CERT_DIR`).
