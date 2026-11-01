# edgeforge-python

Native Python SDK for [EdgeForge](../..), built with [PyO3](https://pyo3.rs).
Ships as prebuilt wheels (cp39–cp313, abi3) via [maturin](https://www.maturin.rs/).

## Install

```bash
pip install edgeforge
```

## Quick Start

```python
import edgeforge

# Create a client (defaults to http://127.0.0.1:11434/v1)
client = edgeforge.Client(api_key="my-secret")

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
```

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

#### `ChatResponse`
- `.text: str` — generated text
- `.model: str` — model ID
- `.tokens_generated: int` — token count
- `.finish_reason: str` — "stop" or "length"

#### `ModelInfo`
- `.id: str`
- `.object: str` — always "model"
- `.created: int` — Unix timestamp
- `.owned_by: str` — publisher

#### `HealthStatus`
- `.status: str` — "ok" or "error"
- `.version: str`

#### `ImportResult`
- `.model_id: str`
- `.status: str` — "accepted"
- `.path: str`

#### `EmbeddingResult`
- `.embedding: list[float]`
- `.model: str`
- `.token_count: int`

#### `SearchHit`
- `.source_id: str`
- `.source_uri: str`
- `.chunk_idx: int | None`
- `.content: str`
- `.score: float` — cosine similarity in [-1, 1]
- `.model_id: str`

#### `FileUploadResult`
- `.id: str`
- `.status: str` — "processed"
- `.chunks: int`
- `.stored: int`
- `.bytes: int`
- `.filename: str | None`
- `.model_id: str`

## Building from Source

### Prerequisites

- Rust 1.82+
- Python 3.9+ with dev headers
- [maturin](https://www.maturin.rs/): `pip install maturin`

### Build the wheel

```bash
# From the workspace root:
maturin build --release -m crates/edgeforge-sdk-python/Cargo.toml

# Or develop mode (installs into the current Python env):
maturin develop -m crates/edgeforge-sdk-python/Cargo.toml
```

### Run the tests

```bash
# Python tests (exercise the pure-Python fallback against a mock server)
cd python && python -m pytest tests/test_sdk.py -v

# Rust tests (serde DTO deserialization)
cargo test -p edgeforge-sdk-python
```

## Fallback Mode

If the compiled native extension is unavailable (e.g. the wheel wasn't
installed for your platform), the SDK automatically falls back to a
pure-Python implementation (`edgeforge/_fallback.py`) that uses
`urllib.request`. The fallback has the same API but is slower (no
connection pooling, no native JSON parsing).

You can check which mode is active:

```python
import edgeforge
print(f"Native extension: {edgeforge._NATIVE}")
```

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

## License

Apache-2.0
