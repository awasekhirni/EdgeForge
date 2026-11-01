# EdgeForge — Phase 16: RAG Embeddings Pipeline Setup Guide

**Version:** 16.0
**Date:** 2026-11-01

This guide covers the RAG (Retrieval-Augmented Generation) embeddings
pipeline: generating embedding vectors, ingesting files, and performing
similarity search.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: Generate Embeddings](#quick-start-generate-embeddings)
3. [Quick Start: RAG File Ingestion](#quick-start-rag-file-ingestion)
4. [Quick Start: Similarity Search](#quick-start-similarity-search)
5. [Using Embeddings Directly (Rust)](#using-embeddings-directly-rust)
6. [HashingEmbedder Configuration](#hashingembedder-configuration)
7. [End-to-End RAG Workflow](#end-to-end-rag-workflow)
8. [API Reference](#api-reference)
9. [Database Schema](#database-schema)
10. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

EdgeForge's RAG pipeline has three components:

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

| Component | Crate | Purpose |
|---|---|---|
| `HashingEmbedder` | edgeforge-runtime | Pure-Rust feature-hashing embedder (no neural network needed) |
| `EmbeddingEngine` trait | edgeforge-runtime | L4 abstraction (swap for a neural embedder later) |
| `Embeddings` ORM | edgeforge-orchestration | Store/search vectors in SQLite |
| `/v1/embeddings` | edgeforge-api | Generate embedding vectors |
| `/v1/embeddings/search` | edgeforge-api | Similarity search |
| `/v1/files` | edgeforge-api | RAG file ingestion (chunk + embed + store) |

### Why HashingEmbedder?

`HashingEmbedder` uses the **feature-hashing trick** (also called the "hashing
trick"): each word and character-bigram in the input is hashed into a
fixed-dimensional vector with sign-hashing to reduce collisions. The output
is L2-normalised.

This is the same design used by:
- Facebook's fastText (hashing-based fallback mode)
- scikit-learn's `FeatureHasher`
- Vowpal Wabbit

It produces **deterministic, real, non-zero vectors** that support meaningful
cosine-similarity search. Two texts sharing many words will have high
similarity. It's not a neural network, but it's a working RAG pipeline that
requires zero external dependencies.

---

## Quick Start: Generate Embeddings

### Prerequisites

- EdgeForge API server running (`edgeforge serve`)
- A model imported into the registry (`edgeforge model import --path ...`)

### Steps

```bash
# 1. Start the API server
./target/release/edgeforge serve &

# 2. Generate an embedding for a single text
curl -s http://127.0.0.1:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "model": "edgeforge-hashing-768",
    "input": "the quick brown fox jumps over the lazy dog"
  }' | python3 -m json.tool

# Response:
# {
#     "object": "list",
#     "data": [
#         {
#             "object": "embedding",
#             "index": 0,
#             "embedding": [0.0, 0.0312, -0.0456, 0.0789, ...]  // 768 floats
#         }
#     ],
#     "model": "edgeforge-hashing-768",
#     "usage": {
#         "prompt_tokens": 7,
#         "total_tokens": 7
#     }
# }

# 3. Generate embeddings for multiple texts
curl -s http://127.0.0.1:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "model": "edgeforge-hashing-768",
    "input": ["hello world", "foo bar baz"]
  }' | python3 -m json.tool
```

---

## Quick Start: RAG File Ingestion

```bash
# 1. Upload a text file for RAG ingestion
echo "The EdgeForge platform is designed for air-gapped inference.
It uses llama.cpp as the primary inference engine via FFI.
The SsdEngine is a pure-Rust fallback that works without llama.cpp.
Embeddings are produced by the HashingEmbedder using feature hashing.
RAG retrieval uses cosine similarity over stored vectors." > /tmp/doc.txt

curl -s http://127.0.0.1:11434/v1/files \
  -F "file=@/tmp/doc.txt" | python3 -m json.tool

# Response:
# {
#     "id": "file-01J9XXXX...",
#     "status": "processed",
#     "chunks": 5,
#     "stored": 5,
#     "bytes": 312,
#     "filename": "doc.txt",
#     "model_id": "edgeforge-hashing-768"
# }
```

The file is automatically:
1. Read and converted to UTF-8 text
2. Chunked into ~2KB pieces at word boundaries
3. Each chunk embedded using the HashingEmbedder
4. Each chunk + vector stored in the `embedding_sources` + `embedding_vectors` tables

---

## Quick Start: Similarity Search

```bash
# Search for chunks similar to a query
curl -s http://127.0.0.1:11434/v1/embeddings/search \
  -H "Content-Type: application/json" \
  -d '{
    "model": "edgeforge-hashing-768",
    "query": "What does the SsdEngine do?",
    "top_k": 3
  }' | python3 -m json.tool

# Response:
# {
#     "object": "list",
#     "model": "edgeforge-hashing-768",
#     "results": [
#         {
#             "source_id": "01J9XXXX...",
#             "source_uri": "doc.txt",
#             "chunk_idx": 2,
#             "content": "The SsdEngine is a pure-Rust fallback that works without llama.cpp.",
#             "score": 0.823,
#             "model_id": "edgeforge-hashing-768"
#         },
#         ...
#     ]
# }
```

---

## Using Embeddings Directly (Rust)

```rust
use edgeforge_runtime::{HashingEmbedder, EmbeddingEngine, cosine_similarity};

let embedder = HashingEmbedder::new(); // dim=768

// Generate an embedding
let v1 = embedder.embed("the quick brown fox");
let v2 = embedder.embed("the quick brown fox jumps");
assert_eq!(v1.len(), embedder.dim()); // 768

// Compute similarity
let sim = cosine_similarity(&v1, &v2);
assert!(sim > 0.7); // similar texts → high similarity
```

### Using the Embeddings ORM

```rust
use edgeforge_orchestration::{Embeddings, StoreChunkParams};
use edgeforge_runtime::{HashingEmbedder, EmbeddingEngine};

let orm = Embeddings::new(pool);
let embedder = HashingEmbedder::new();
let model_id = embedder.model_id().to_string();

// Store a chunk
let vector = embedder.embed("the quick brown fox");
let source_id = orm.store_chunk(&StoreChunkParams {
    source_type: "file".to_string(),
    source_uri: "doc.txt".to_string(),
    chunk_idx: Some(0),
    content: "the quick brown fox".to_string(),
    model_id: model_id.clone(),
    vector,
}).await?;

// Search for similar chunks
let query_vec = embedder.embed("quick fox");
let results = orm.search(&query_vec, &model_id, 5).await?;
for r in &results {
    println!("score={:.3} content={}", r.score, r.content);
}
```

---

## HashingEmbedder Configuration

| Field | Default | Purpose |
|---|---|---|
| `dim` | 768 | Vector dimensionality (768 matches many transformer models) |
| `use_word_unigrams` | true | Include word unigrams as features |
| `use_char_bigrams` | true | Include character bigrams (helps with morphology/typos) |
| `model_id` | `"edgeforge-hashing-768"` | Model ID reported to the API |

```rust
use edgeforge_runtime::{HashingEmbedder, HashingEmbedderConfig};

let embedder = HashingEmbedder::with_config(HashingEmbedderConfig {
    dim: 512,
    use_char_bigrams: false,
    model_id: "my-embedder".to_string(),
    ..HashingEmbedderConfig::default()
});
```

### Custom Embedder

To use a different embedder (e.g. a real neural embedder via LlamaBackend),
implement the `EmbeddingEngine` trait and pass it to `AppState::with_embedder`:

```rust
use edgeforge_api::AppState;
use std::sync::Arc;

let state = AppState::with_embedder(
    models, jobs, embeddings,
    Arc::new(my_neural_embedder),
    require_api_key, rate_limit,
);
```

---

## End-to-End RAG Workflow

```bash
# 1. Start the server
./target/release/edgeforge serve &

# 2. Import a model (required for chat/completions)
./target/release/edgeforge model import --path /path/to/model.gguf

# 3. Upload documents for RAG
curl -s http://127.0.0.1:11434/v1/files -F "file=@doc1.txt"
curl -s http://127.0.0.1:11434/v1/files -F "file=@doc2.txt"

# 4. Search for relevant chunks
SEARCH_RESULTS=$(curl -s http://127.0.0.1:11434/v1/embeddings/search \
  -H "Content-Type: application/json" \
  -d '{"model":"edgeforge-hashing-768","query":"what is inference?","top_k":3}')

# 5. Use the search results as context for a chat completion
# (Extract the content from SEARCH_RESULTS and include it in the prompt)
curl -s http://127.0.0.1:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d "{
    \"model\": \"YOUR_MODEL_ID\",
    \"messages\": [
      {\"role\": \"system\", \"content\": \"Answer using this context: $SEARCH_RESULTS\"},
      {\"role\": \"user\", \"content\": \"What is inference?\"}
    ]
  }"
```

---

## API Reference

### POST /v1/embeddings

Generate embedding vectors for one or more texts.

**Request:**
```json
{
  "model": "edgeforge-hashing-768",
  "input": "text to embed"
  // OR: "input": ["text1", "text2"]
}
```

**Response:**
```json
{
  "object": "list",
  "data": [
    { "object": "embedding", "index": 0, "embedding": [0.1, -0.2, ...] }
  ],
  "model": "edgeforge-hashing-768",
  "usage": { "prompt_tokens": 3, "total_tokens": 3 }
}
```

### POST /v1/embeddings/search

Search for the top-k most similar stored chunks.

**Request:**
```json
{
  "model": "edgeforge-hashing-768",
  "query": "search query text",
  "top_k": 5
}
```

**Response:**
```json
{
  "object": "list",
  "model": "edgeforge-hashing-768",
  "results": [
    {
      "source_id": "01J9...",
      "source_uri": "doc.txt",
      "chunk_idx": 0,
      "content": "matched text...",
      "score": 0.823,
      "model_id": "edgeforge-hashing-768"
    }
  ]
}
```

### POST /v1/files

Upload a file for RAG ingestion (chunk + embed + store).

**Request:** multipart form data with a `file` field.

**Response:**
```json
{
  "id": "file-01J9...",
  "status": "processed",
  "chunks": 5,
  "stored": 5,
  "bytes": 312,
  "filename": "doc.txt",
  "model_id": "edgeforge-hashing-768"
}
```

### POST /v1/completions

Generate a text completion (legacy, non-chat format). Wired to `SsdEngine`.

**Request:**
```json
{
  "model": "YOUR_MODEL_ID",
  "prompt": "Once upon a time",
  "max_tokens": 128,
  "temperature": 0.7,
  "top_p": 0.9,
  "seed": 42,
  "stop": ["\n"]
}
```

**Response:**
```json
{
  "id": "cmpl-01J9...",
  "object": "text_completion",
  "created": 1730000000,
  "model": "YOUR_MODEL_ID",
  "choices": [
    { "index": 0, "text": "...", "finish_reason": "stop" }
  ],
  "usage": { "prompt_tokens": 3, "completion_tokens": 128, "total_tokens": 131 }
}
```

---

## Database Schema

### embedding_sources (migration 0007)

```sql
CREATE TABLE embedding_sources (
    id          TEXT PRIMARY KEY,
    source_type TEXT NOT NULL CHECK (source_type IN ('file','chunk','url')),
    source_uri  TEXT NOT NULL,
    chunk_idx   INTEGER,
    content     TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    embedded_at INTEGER NOT NULL
);
```

### embedding_vectors (migration 0017)

```sql
CREATE TABLE embedding_vectors (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    dim         INTEGER NOT NULL CHECK (dim > 0),
    vector      BLOB NOT NULL,        -- little-endian f32 bytes
    norm        REAL NOT NULL,        -- precomputed L2 norm
    created_at  INTEGER NOT NULL
);
CREATE INDEX idx_embedding_vectors_model  ON embedding_vectors(model_id);
CREATE INDEX idx_embedding_vectors_source ON embedding_vectors(source_id);
```

---

## Troubleshooting

### Embeddings are all zeros

You're using an old build. The embeddings endpoint now uses `HashingEmbedder`
which produces real, non-zero vectors. Rebuild:

```bash
cargo build --release --bin edgeforge
```

### Search returns no results

No chunks have been stored for the requested model_id. Upload a file first:

```bash
curl -s http://127.0.0.1:11434/v1/files -F "file=@doc.txt"
```

Then verify the model_id matches:

```bash
# The /v1/files response includes the model_id used.
# Use the same model_id in /v1/embeddings/search.
```

### "model not found" on /v1/completions

The `/v1/completions` endpoint requires the model to exist in the registry.
Import it first:

```bash
./target/release/edgeforge model import --path /path/to/model.gguf
```

### File upload stores fewer chunks than expected

Some chunks may have failed to embed/store. Check the server logs for
`"failed to store chunk"` warnings. The response's `stored` field shows how
many chunks were successfully stored (may be less than `chunks`).

### Similarity scores are low

The `HashingEmbedder` is a feature-hashing embedder, not a neural network.
For higher-quality embeddings, swap in a real neural embedder by
implementing the `EmbeddingEngine` trait and passing it to
`AppState::with_embedder`.

### How do I use a different embedding dimension?

```rust
use edgeforge_runtime::{HashingEmbedder, HashingEmbedderConfig};

let embedder = HashingEmbedder::with_config(HashingEmbedderConfig {
    dim: 1536, // OpenAI-compatible dimension
    ..HashingEmbedderConfig::default()
});
```

Then pass it to `AppState::with_embedder` in your `serve` command.
