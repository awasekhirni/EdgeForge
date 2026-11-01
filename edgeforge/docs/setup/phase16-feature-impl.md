# EdgeForge — Phase 16: RAG Embeddings Pipeline Implementation

**Date:** 2026-11-01
**Status:** Complete
**Feature:** F-DEV-06 (RAG Embeddings + File Ingestion + Similarity Search)
**Files changed:** 11 source files + 2 new doc/test files

---

## Summary

Three API endpoints were returning stub/placeholder data, making the RAG
(Retrieval-Augmented Generation) pipeline completely non-functional:

1. **`POST /v1/embeddings`** returned zero vectors (`vec![0.0; 1536]`) —
   useless for any similarity computation.
2. **`POST /v1/files`** chunked uploaded files but did not embed or store
   the chunks — no RAG ingestion actually happened.
3. **`POST /v1/completions`** returned a hardcoded stub string
   (`"Text completion endpoint — Phase 11 stub..."`) — no real completion.

There was also no similarity-search endpoint.

All four gaps are now fixed with a complete, working RAG pipeline:

- A pure-Rust `HashingEmbedder` produces real, deterministic, L2-normalised
  embedding vectors using the feature-hashing trick (word unigrams + char
  bigrams). No neural network, no external dependencies — but vectors
  support meaningful cosine-similarity search.
- An `Embeddings` ORM stores chunks + vectors in SQLite
  (`embedding_sources` + new `embedding_vectors` table) and performs
  cosine-similarity search.
- `/v1/embeddings` now returns real vectors.
- `/v1/files` now chunks → embeds → stores each chunk.
- New `POST /v1/embeddings/search` endpoint performs similarity search.
- `/v1/completions` now generates real completions via `SsdEngine`.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     RAG Pipeline                             │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐   │
│  │ HashingEmbedder │──▶│ Embeddings   │──▶│  SQLite      │   │
│  │ (pure Rust)    │   │ ORM          │    │  (vectors)   │   │
│  └──────────────┘    └──────────────┘    └──────────────┘   │
│         ▲                     ▲                    │         │
│         │                     │                    │         │
│  ┌──────┴──────┐       ┌──────┴──────┐       ┌──────▼──────┐ │
│  │ /v1/        │       │ /v1/files   │       │ /v1/        │ │
│  │ embeddings  │       │ (upload +   │       │ embeddings/ │ │
│  │ (generate)  │       │  chunk +    │       │ search      │ │
│  │             │       │  embed +    │       │ (retrieve)  │ │
│  └─────────────┘       │  store)     │       └─────────────┘ │
│                        └─────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Changes

### New Files (4)

| File | Purpose |
|---|---|
| `crates/edgeforge-runtime/src/embedding.rs` | `EmbeddingEngine` trait + `HashingEmbedder` + vector math utilities (17 unit tests) |
| `crates/edgeforge-orchestration/src/embeddings.rs` | `Embeddings` ORM: store_chunk, search, list_by_model, list_by_source, delete_by_source, count |
| `crates/edgeforge-orchestration/tests/embeddings_pipeline.rs` | 12 integration tests (ORM + end-to-end RAG pipeline) |
| `resources/migrations/0017_embedding_vectors.sql` | New `embedding_vectors` table for vector BLOB storage |
| `docs/setup/phase16-rag-embeddings.md` | Complete setup guide with API reference |

### Modified Files (7)

| File | Change |
|---|---|
| `crates/edgeforge-runtime/src/lib.rs` | Export `embedding` module + `EmbeddingEngine`, `HashingEmbedder`, `HashingEmbedderConfig`, `cosine_similarity`, `dot_product`, `l2_norm`, `encode_vector_f32le`, `decode_vector_f32le` |
| `crates/edgeforge-orchestration/src/lib.rs` | Export `embeddings` module + `Embeddings`, `EmbeddingSourceRow`, `SearchResult`, `StoreChunkParams` |
| `crates/edgeforge-orchestration/Cargo.toml` | Add `tokio` dev-dependency (for async integration tests) |
| `crates/edgeforge-api/src/state.rs` | Add `embeddings: Arc<Embeddings>` + `embedder: Arc<dyn EmbeddingEngine>` to `AppState`; new `with_embedder()` constructor |
| `crates/edgeforge-api/src/lib.rs` | Updated module docs with full endpoint table |
| `crates/edgeforge-api/src/routes/embeddings.rs` | Wire to real `HashingEmbedder`; add `POST /v1/embeddings/search` endpoint; 6 unit tests |
| `crates/edgeforge-api/src/routes/files.rs` | Chunk + embed + store each chunk via `Embeddings` ORM; 6 unit tests |
| `crates/edgeforge-api/src/routes/completions.rs` | Wire to `SsdEngine` (replace hardcoded stub string); 5 unit tests |
| `crates/edgeforge-cli/Cargo.toml` | Add `edgeforge-runtime` dependency (for `HashingEmbedder`) |
| `crates/edgeforge-cli/src/commands/serve.rs` | Pass `Embeddings::new(pool)` to `AppState::new` |

---

## Detailed Design

### HashingEmbedder (edgeforge-runtime/src/embedding.rs)

**Algorithm:** Feature-hashing (the "hashing trick").

1. Tokenize the text into word unigrams (lowercase, alphanumeric) and
   character bigrams (with `<` and `>` boundary markers).
2. For each feature, compute an FNV-1a 64-bit hash → index in `[0, dim)`.
3. Compute a second hash (seeded with the first) → sign in `{-1, +1}`.
4. Accumulate `sign * weight` at each index (words: weight 1.0, char-bigrams: weight 0.5).
5. L2-normalise the resulting vector.

**Properties:**
- Deterministic: same input → same vector (no RNG)
- Real, non-zero vectors (unlike the old stub)
- L2-normalised → cosine similarity = dot product
- Two texts sharing many words hash to the same indices → high similarity
- Sign-hashing reduces collision impact (colliding features cancel 50% of the time)

**Configuration (`HashingEmbedderConfig`):**
- `dim: 768` (default; matches many transformer models)
- `use_word_unigrams: true`
- `use_char_bigrams: true`
- `model_id: "edgeforge-hashing-768"`

### Vector Math Utilities

- `l2_norm(v) -> f32` — Euclidean norm
- `dot_product(a, b) -> f32` — elementwise dot product
- `cosine_similarity(a, b) -> f32` — cosine sim in `[-1, 1]`; handles zero vectors and mismatched lengths
- `encode_vector_f32le(v) -> Vec<u8>` — LE f32 bytes for SQLite BLOB
- `decode_vector_f32le(bytes) -> Vec<f32>` — decode BLOB back to vector

### Embeddings ORM (edgeforge-orchestration/src/embeddings.rs)

**Tables:**
- `embedding_sources` (migration 0007) — text chunks with metadata
- `embedding_vectors` (migration 0017, new) — dense vectors as LE f32 BLOBs

**Methods:**
- `store_chunk(params) -> String` — insert chunk + vector in a single transaction
- `search(query_vec, model_id, top_k) -> Vec<SearchResult>` — cosine-similarity search
- `list_by_model(model_id) -> Vec<EmbeddingSourceRow>`
- `list_by_source(source_uri) -> Vec<EmbeddingSourceRow>`
- `delete_by_source(source_uri) -> u64` — delete all chunks + vectors for a source
- `count(model_id) -> i64`

**Search Algorithm:**
1. `SELECT` all vectors + sources for the given `model_id` (JOIN)
2. Decode each vector BLOB into `Vec<f32>`
3. Compute cosine similarity against the query vector
4. Sort by similarity descending
5. Take `top_k`

**Performance:** O(n) where n = stored vectors for the model. Fine for
small-to-medium corpora (thousands of chunks). For production scale, swap
in `sqlite-vss` or a dedicated vector DB — the `EmbeddingEngine` trait
makes this a drop-in replacement.

### API Endpoints

#### POST /v1/embeddings (was stub, now working)

Generates real vectors via the configured `EmbeddingEngine`.

```json
// Request
{ "model": "edgeforge-hashing-768", "input": "hello world" }

// Response
{ "object": "list", "data": [{ "object": "embedding", "index": 0, "embedding": [0.1, ...] }], ... }
```

#### POST /v1/embeddings/search (new endpoint)

Embeds the query, searches the vector store, returns ranked results.

```json
// Request
{ "model": "edgeforge-hashing-768", "query": "what is inference?", "top_k": 5 }

// Response
{ "object": "list", "results": [{ "source_id": "...", "content": "...", "score": 0.82, ... }] }
```

#### POST /v1/files (was stub, now working)

Chunks → embeds → stores each chunk.

```json
// Response
{ "id": "file-...", "status": "processed", "chunks": 5, "stored": 5, "bytes": 312, ... }
```

#### POST /v1/completions (was stub, now working)

Generates real completions via `SsdEngine`.

```json
// Request
{ "model": "YOUR_MODEL_ID", "prompt": "Once upon a time", "max_tokens": 128 }

// Response
{ "id": "cmpl-...", "choices": [{ "text": "...", "finish_reason": "stop" }], ... }
```

---

## Tests

### Unit Tests

| File | Tests | Coverage |
|---|---|---|
| `embedding.rs` | 17 | dimension, non-zero, L2-normalised, determinism, similar/dissimilar similarity, empty text, mismatched lengths, zero vectors, identical vectors, dot product, encode/decode roundtrip, invalid byte count, char bigrams, word unigrams, model_id, disable char bigrams |
| `embeddings.rs` (API) | 6 | request single/many, response serialize, search request defaults/custom top_k, search response serialize |
| `files.rs` (API) | 6 | chunk short/long/empty/zero-size/long-word, response serialize |
| `completions.rs` (API) | 5 | request deserialize/defaults/seed+stop, response serialize, finish reason |

### Integration Tests

| File | Tests | Coverage |
|---|---|---|
| `embeddings_pipeline.rs` | 12 | embedder non-zero/deterministic/similar/dissimilar, ORM store+search, search ranking, model_id filter, list_by_model, list_by_source, delete_by_source, count, empty DB, nonexistent source, end-to-end RAG pipeline |

**Total: 46 new tests.**

---

## Validation

### Migrations

```bash
python3 scripts/validate_migrations.py resources/migrations
# Expect: 17 migrations applied, 25 tables, sample INSERTs succeed
```

### Embedder + ORM

```bash
cargo test -p edgeforge-runtime embedding
cargo test -p edgeforge-orchestration embeddings_pipeline
```

### API endpoints

```bash
cargo test -p edgeforge-api -- embeddings files completions
```

### Manual end-to-end

```bash
# Start server
./target/release/edgeforge serve &

# Upload a document
echo "The quick brown fox jumps over the lazy dog." > /tmp/doc.txt
curl -s http://127.0.0.1:11434/v1/files -F "file=@/tmp/doc.txt"

# Search for a chunk
curl -s http://127.0.0.1:11434/v1/embeddings/search \
  -H "Content-Type: application/json" \
  -d '{"model":"edgeforge-hashing-768","query":"fox","top_k":3}'
```

---

## Risk Assessment

- **HashingEmbedder is not a neural network.** It produces deterministic,
  real vectors that support meaningful similarity search, but the quality is
  lower than a transformer-based embedder. For production RAG, swap in a
  real neural embedder by implementing the `EmbeddingEngine` trait.

- **Search is O(n).** Fine for thousands of chunks. For millions, use
  `sqlite-vss` or a dedicated vector DB. The `EmbeddingEngine` trait and
  `Embeddings` ORM make this a drop-in replacement.

- **`/v1/completions` uses SsdEngine.** The SsdEngine is a Markov-chain
  sampler (see Phase 15 P0 fixes). It produces deterministic, prompt-derived
  completions that exercise the full inference pipeline. For real LLM
  completions, enable the `llama-backend` feature.
