# EdgeForge — Phase 19: Python SDK Implementation

**Version:** 19.0
**Date:** 2026-11-01

This guide covers the EdgeForge Python SDK: installation, usage, building
from source, and the pure-Python fallback.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: Install](#quick-start-install)
3. [Quick Start: Usage](#quick-start-usage)
4. [Building from Source](#building-from-source)
5. [Pure-Python Fallback](#pure-python-fallback)
6. [API Reference](#api-reference)
7. [Platform Support](#platform-support)
8. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

The Python SDK is a native extension built with [PyO3](https://pyo3.rs),
shipped as prebuilt wheels via [maturin](https://www.maturin.rs/).

```
┌─────────────────────────────────────────────────────────────┐
│  Python: import edgeforge                                   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  edgeforge/__init__.py                                │  │
│  │  ┌─────────────────┐    ┌─────────────────────────┐  │  │
│  │  │ Try native ext  │───▶│ Pure-Python fallback    │  │  │
│  │  │ (PyO3 cdylib)   │    │ (_fallback.py via       │  │  │
│  │  │                 │    │  urllib.request)        │  │  │
│  │  └─────────────────┘    └─────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  HTTP (reqwest blocking / urllib)                           │
└─────────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  EdgeForge API Server (edgeforge serve)                     │
│  GET /health, GET/POST /v1/*                                 │
└─────────────────────────────────────────────────────────────┘
```

### Components

| Component | Location | Purpose |
|---|---|---|
| PyO3 extension | `crates/edgeforge-sdk-python/src/lib.rs` | Native Python module (`#[pymodule]`, `#[pyclass]`, `#[pymethods]`) |
| Cargo manifest | `crates/edgeforge-sdk-python/Cargo.toml` | PyO3 + reqwest + abi3-py39 config |
| Python package | `python/edgeforge/__init__.py` | Tries native ext, falls back to pure Python |
| Pure-Python fallback | `python/edgeforge/_fallback.py` | Same API, `urllib.request`-based |
| Build config | `pyproject.toml` | maturin build backend + wheel metadata |
| Python tests | `python/tests/test_sdk.py` | 18 tests against a mock server |
| Rust tests | `crates/edgeforge-sdk-python/src/lib.rs` | 9 serde DTO deserialization tests |

---

## Quick Start: Install

```bash
pip install edgeforge
```

Or from source:

```bash
git clone https://github.com/your-org/edgeforge.git
cd edgeforge
pip install maturin
maturin develop --release -m crates/edgeforge-sdk-python/Cargo.toml
```

---

## Quick Start: Usage

```python
import edgeforge

# Create a client
client = edgeforge.Client(
    base_url="http://127.0.0.1:11434/v1",
    api_key="my-secret",  # optional, only if the server requires it
)

# Check server health
health = client.health()
print(f"Status: {health.status}, Version: {health.version}")

# List models
for model in client.list_models():
    print(f"  {model.id} (owned by {model.owned_by})")

# Chat completion
resp = client.chat(
    model="YOUR_MODEL_ID",
    prompt="Hello, world!",
    max_tokens=128,
    temperature=0.7,
)
print(f"Response: {resp.text}")
print(f"Tokens: {resp.tokens_generated}, Finish: {resp.finish_reason}")

# Generate an embedding
emb = client.embed("the quick brown fox")
print(f"Embedding dim: {len(emb.embedding)}")

# RAG: upload a file, then search
client.upload_file("/path/to/doc.txt")
hits = client.search("what is in the document?", top_k=3)
for hit in hits:
    print(f"  score={hit.score:.3f}: {hit.content[:80]}")

# Import a model (server-side path)
result = client.import_model("/path/to/model.gguf")
print(f"Imported: {result.model_id} ({result.status})")
```

---

## Building from Source

### Prerequisites

- Rust 1.82+
- Python 3.9+ with dev headers
- [maturin](https://www.maturin.rs/): `pip install maturin`

### Build the wheel

```bash
# From the workspace root:
maturin build --release -m crates/edgeforge-sdk-python/Cargo.toml

# The wheel will be in target/wheels/
pip install target/wheels/edgeforge-*.whl
```

### Develop mode (installs into the current Python env)

```bash
maturin develop --release -m crates/edgeforge-sdk-python/Cargo.toml
```

### Cross-compiling for other platforms

```bash
# macOS universal2
maturin build --release --target universal2-apple-darwin -m crates/edgeforge-sdk-python/Cargo.toml

# Linux manylinux x86_64 (requires Docker or zigbuild)
maturin build --release --target x86_64-unknown-linux-gnu -m crates/edgeforge-sdk-python/Cargo.toml

# Windows x64
maturin build --release --target x86_64-pc-windows-msvc -m crates/edgeforge-sdk-python/Cargo.toml
```

### Run the tests

```bash
# Python tests (exercise the pure-Python fallback against a mock server)
cd python && python -m pytest tests/test_sdk.py -v
# Expected: 18 passed

# Rust tests (serde DTO deserialization)
cargo test -p edgeforge-sdk-python
# Expected: 9 passed
```

---

## Pure-Python Fallback

If the compiled native extension is unavailable (e.g. the wheel wasn't
installed for your platform, or you're running from source without
`maturin develop`), the SDK automatically falls back to a pure-Python
implementation (`edgeforge/_fallback.py`) that uses `urllib.request`.

The fallback has the **same API** as the native extension, so your code
doesn't need to change. It's slower (no connection pooling, no native JSON
parsing) but fully functional.

### Check which mode is active

```python
import edgeforge
print(f"Native extension: {edgeforge._NATIVE}")  # True or False
```

### When the fallback is used

- The wheel wasn't installed for your platform (e.g. an unsupported Python version)
- You're running from source without `maturin develop`
- The compiled extension failed to load (e.g. missing shared library)

### When the native extension is used

- You installed via `pip install edgeforge` (the wheel matches your platform)
- You ran `maturin develop` from source

---

## API Reference

### `edgeforge.Client(base_url="http://127.0.0.1:11434/v1", api_key=None, timeout=30.0)`

Create a new client.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `base_url` | `str` | `"http://127.0.0.1:11434/v1"` | API server base URL |
| `api_key` | `str \| None` | `None` | API key (sent as `Authorization: Bearer <key>`) |
| `timeout` | `float` | `30.0` | Request timeout in seconds |

### Methods

| Method | Returns | Description |
|---|---|---|
| `health()` | `HealthStatus` | Check server health |
| `list_models()` | `list[ModelInfo]` | List imported models |
| `get_model(model_id)` | `ModelInfo` | Get a single model by ID |
| `chat(model, prompt, max_tokens=512, temperature=0.7, top_p=0.9, seed=None)` | `ChatResponse` | Chat completion |
| `import_model(path)` | `ImportResult` | Import a model (server-side path) |
| `embed(text, model="edgeforge-hashing-768")` | `EmbeddingResult` | Generate an embedding |
| `search(query, top_k=5, model="edgeforge-hashing-768")` | `list[SearchHit]` | RAG similarity search |
| `upload_file(file_path)` | `FileUploadResult` | Upload a file for RAG ingestion |

### Response Types

See the [README](../../crates/edgeforge-sdk-python/README.md#response-types)
for the full response type field reference.

---

## Platform Support

| Platform | Wheel | Status |
|---|---|---|
| macOS x86_64 | `edgeforge-*-macosx_10_12_x86_64.whl` | ✅ |
| macOS arm64 | `edgeforge-*-macosx_11_0_arm64.whl` | ✅ |
| Linux x86_64 | `edgeforge-*-manylinux_2_17_x86_64.whl` | ✅ |
| Linux aarch64 | `edgeforge-*-manylinux_2_17_aarch64.whl` | ✅ |
| Windows x64 | `edgeforge-*-win_amd64.whl` | ✅ |

All wheels use the `abi3` stable ABI, so a single wheel per platform works
for Python 3.9 through 3.13+.

---

## Troubleshooting

### "ImportError: No module named 'edgeforge.edgeforge_python'"

The compiled native extension is not available. The SDK will fall back to
the pure-Python implementation automatically. To use the native extension:

```bash
pip install edgeforge  # or
maturin develop -m crates/edgeforge-sdk-python/Cargo.toml
```

### "RuntimeError: chat request failed: ..."

The SDK can't reach the server. Check:
1. The server is running: `curl http://127.0.0.1:11434/health`
2. The `base_url` is correct
3. The API key (if required) is set

### "RuntimeError: chat returned HTTP 401"

Authentication failed. The server has `require_api_key=true` but you either:
- Didn't set `api_key` in the client constructor
- Set the wrong key

### "RuntimeError: chat returned HTTP 429"

Rate limited. The server's rate limit (default: 600 req/min per IP) was
exceeded. Wait for the bucket to refill or increase the limit in the server
config.

### The SDK is returning hardcoded strings

This was a **Phase 18 bug** (the SDK was a plain Rust stub that returned
`"Hello from EdgeForge! (model=...)"` instead of making real HTTP requests).
It is fixed: the SDK now uses `reqwest` to make real HTTP calls to the API
server. Update to the latest build:

```bash
pip install --force-reinstall edgeforge
```

### How do I verify the native extension is loaded?

```python
import edgeforge
print(f"Native: {edgeforge._NATIVE}")  # True if the PyO3 extension is loaded
```

### How do I use the SDK with a custom CA certificate?

The native extension uses `reqwest`, which respects the `SSL_CERT_FILE` and
`SSL_CERT_DIR` environment variables. The pure-Python fallback uses
`urllib.request`, which also respects these variables.
