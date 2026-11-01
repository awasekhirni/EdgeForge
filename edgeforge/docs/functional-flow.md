# EdgeForge — Complete Functional Flow Documentation

**Version:** 14.0
**Date:** 2026-10-31

This document captures the complete functional flow of EdgeForge from beginning to end — from user input to token output.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Startup Flow](#2-startup-flow)
3. [Model Import Flow](#3-model-import-flow)
4. [Chat Completion Flow (Non-Streaming)](#4-chat-completion-flow-non-streaming)
5. [Chat Completion Flow (SSE Streaming)](#5-chat-completion-flow-sse-streaming)
6. [Inference Engine Flow](#6-inference-engine-flow)
7. [Storage Tier Flow](#7-storage-tier-flow)
8. [Streaming Graph Flow](#8-streaming-graph-flow)
9. [Multi-Horizon Speculation Flow](#9-multi-horizon-speculation-flow)
10. [Distributed Pool Flow](#10-distributed-pool-flow)
11. [Prompt Security Flow](#11-prompt-security-flow)
12. [Autonomic Self-Healing Flow](#12-autonomic-self-healing-flow)
13. [Monitoring & Health Flow](#13-monitoring--health-flow)
14. [Function Call Logging Flow](#14-function-call-logging-flow)
15. [Data Persistence Flow](#15-data-persistence-flow)

---

## 1. System Overview

EdgeForge is an 11-layer system with 18 Rust crates:

```
User → CLI/API → Orchestration → Inference Engine → Storage → SSD
                         ↕              ↕             ↕
                    Streaming Graph   Router Hook   Expert Cache
                         ↕              ↕
                    Autonomic Loop   Speculation
                         ↕
                    Prompt Security
```

### Key Components

| Component | Crate | Layer | Purpose |
|---|---|---|---|
| CLI | edgeforge-cli | L6 | Command-line interface |
| API Server | edgeforge-api | L5 | OpenAI-compatible REST + SSE |
| Orchestration | edgeforge-orchestration | L5 | Model lifecycle, jobs, scheduler |
| Inference Engine | edgeforge-runtime | L4 | MockInferenceEngine / LlamaBackend |
| Streaming Graph | edgeforge-graph | L3.5 | Derived control plane (<10µs queries) |
| Storage | edgeforge-storage | L3 | WeightPager, KvCachePager, ExpertCache |
| Networking | edgeforge-net | L2 | QUIC mesh, mDNS, mTLS, bandwidth |
| Crowd | edgeforge-crowd | L2.5 | Reputation, verification, incentives |
| Prompt Security | edgeforge-promptsec | L1.5 | Dual-LLM, spotlighting, guardrails |
| Security | edgeforge-security | L1 | Manifest verifier, crypto-agility |
| Observability | edgeforge-observability | Cross | Tracing + function-call logger |
| Autonomic | edgeforge-autonomic | L5.5 | MAPE-K self-healing loop |
| KPI | edgeforge-kpi | Cross | QPPOs, traceability, carbon-aware |
| Core | edgeforge-core | Cross | Types, errors, config, ControlEvent |

---

## 2. Startup Flow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. User runs: edgeforge serve                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 2. CLI (edgeforge-cli/src/main.rs)                          │
│    ├── Parse CLI args (clap)                                │
│    ├── Load Config (edgeforge-core/src/config.rs)           │
│    │   ├── Read ~/.config/edgeforge/config.toml             │
│    │   ├── Merge env vars (EDGEFORGE_*)                     │
│    │   └── Validate (min_ssd >= 2TB, precision = FP16)     │
│    ├── Init Observability (edgeforge-observability)         │
│    │   ├── tracing subscriber with env filter               │
│    │   └── FunctionLogger initialized                      │
│    └── Dispatch to serve command                            │
│                                                             │
│ 3. Serve Command (edgeforge-cli/src/commands/serve.rs)      │
│    ├── Create data directory if needed                      │
│    ├── Open SQLite database (edgeforge.db)                  │
│    ├── Run 16 SQLx migrations                               │
│    │   ├── 0001_init.sql (meta table)                       │
│    │   ├── 0002_models.sql (model registry)                 │
│    │   ├── 0003_jobs.sql (job queue)                        │
│    │   ├── ... (16 total migrations)                        │
│    │   └── 0016_crypto_inventory.sql                        │
│    ├── Create AppState (Models + Jobs)                      │
│    └── Call edgeforge_api::serve(addr, state)               │
│                                                             │
│ 4. API Server (edgeforge-api/src/server.rs)                 │
│    ├── Build Router with all routes:                        │
│    │   ├── GET  /health                                     │
│    │   ├── GET  /v1/models                                  │
│    │   ├── GET  /v1/models/:id                              │
│    │   ├── POST /v1/chat/completions                        │
│    │   ├── POST /v1/completions                             │
│    │   ├── POST /v1/embeddings                              │
│    │   ├── POST /v1/files                                   │
│    │   └── GET/POST /v1/webhooks                            │
│    ├── Apply middleware:                                    │
│    │   ├── logging_middleware (structured request/response) │
│    │   └── TraceLayer (HTTP tracing)                        │
│    ├── Bind TcpListener to 127.0.0.1:11434                  │
│    └── Enter axum serve loop                                │
│                                                             │
│ 5. Server is ready. Log: "edgeforge API server listening"   │
└─────────────────────────────────────────────────────────────┘
```

### Startup Sequence (Step-by-Step)

| Step | Component | Action | Log Entry |
|---|---|---|---|
| 1 | CLI | Parse `edgeforge serve` | `timestamp - edgeforge-cli - main.rs - main - ENTRY` |
| 2 | Core | Load Config from TOML + env | `timestamp - edgeforge-core - config.rs - Config::load - SUCCESS` |
| 3 | Core | Validate config (SSD ≥ 2TB, FP16 default) | `timestamp - edgeforge-core - config.rs - validate - SUCCESS` |
| 4 | Observability | Init tracing subscriber | `timestamp - edgeforge-observability - subscriber.rs - init - SUCCESS` |
| 5 | CLI | Create data directory | `timestamp - edgeforge-cli - serve.rs - run - ENTRY` |
| 6 | Orchestration | Open SQLite + run migrations | `timestamp - edgeforge-orchestration - db.rs - new - SUCCESS` |
| 7 | API | Build router with all routes | `timestamp - edgeforge-api - server.rs - build_router - SUCCESS` |
| 8 | API | Bind TCP listener | `timestamp - edgeforge-api - server.rs - serve - SUCCESS` |
| 9 | API | Log "listening on 127.0.0.1:11434" | tracing::info! |

---

## 3. Model Import Flow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. User runs: edgeforge model import --path /path/model.gguf│
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 2. CLI Model Command (edgeforge-cli/src/commands/model.rs)  │
│    ├── Open SQLite database + run migrations                │
│    ├── Verify file exists                                   │
│    ├── Get file metadata (size)                             │
│    ├── Compute SHA-256 hash (streaming, 1 MiB chunks)       │
│    │   └── Uses ring::digest::Context                       │
│    ├── Auto-detect precision from filename                  │
│    │   ├── "Q4_K_M" → Q4                                    │
│    │   ├── "FP16" → FP16                                    │
│    │   └── default → FP16                                   │
│    ├── Auto-detect architecture from filename               │
│    │   ├── "mixtral" → MoE                                  │
│    │   ├── "rwkv" → RWKV                                    │
│    │   └── default → Dense                                   │
│    ├── Insert into models table (SQLx)                      │
│    │   └── Returns ModelId (ULID)                           │
│    └── Print: "✅ Model imported successfully!"             │
│        Print: "Model ID: 01J9XXXXXXXXXXXXXXXXXXX"           │
│                                                             │
│ 3. Model is now in the registry.                            │
│    Use with: edgeforge run --model <ID> --prompt "Hello!"   │
│    Or via API: POST /v1/chat/completions                    │
└─────────────────────────────────────────────────────────────┘
```

### Model Import Steps

| Step | Component | Action | Output |
|---|---|---|---|
| 1 | CLI | Parse path argument | Path string |
| 2 | std::fs | Verify file exists | Error if not found |
| 3 | std::fs | Get file metadata | file_size_bytes |
| 4 | ring::digest | Stream SHA-256 hash | [u8; 32] |
| 5 | model.rs | Detect precision from filename | Precision enum |
| 6 | model.rs | Detect architecture from filename | ModelArch enum |
| 7 | SQLx | INSERT INTO models (...) | ModelId (ULID) |
| 8 | CLI | Print success message | Console output |

---

## 4. Chat Completion Flow (Non-Streaming)

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Client sends POST /v1/chat/completions                   │
│    Body: { model: "01J9...", messages: [...], stream: false}│
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 2. API Middleware (edgeforge-api/src/middleware/)           │
│    ├── logging_middleware: log REQUEST (method, URI)        │
│    └── TraceLayer: HTTP tracing                             │
│                                                             │
│ 3. Chat Handler (edgeforge-api/src/routes/chat.rs)          │
│    ├── Parse model ID from request                          │
│    ├── Query models table: state.models.get(mid)            │
│    │   └── Returns ModelRow (name, precision, arch, etc.)   │
│    ├── Build ModelManifest from ModelRow                    │
│    ├── Create MockInferenceEngine                           │
│    ├── engine.load(manifest, ExecutionMode::Local)          │
│    │   └── Returns LoadedModel                              │
│    ├── Build Prompt from request.messages                   │
│    ├── Build GenParams (max_tokens, temp, top_p, seed)      │
│    ├── engine.generate(model, prompt, params, callback)     │
│    │   ├── For each token:                                  │
│    │   │   ├── Derive token from prompt (mock) or           │
│    │   │   │   sample from logits (real engine)             │
│    │   │   ├── Call router_hook.resolve_expert() if installed│
│    │   │   ├── Invoke callback(token, VerificationOutcome)  │
│    │   │   └── Append token text to full_response           │
│    │   └── Returns GenStats (tokens, elapsed_ms, etc.)      │
│    ├── Build ChatCompletionResponse                         │
│    │   ├── id: "chatcmpl-<ULID>"                           │
│    │   ├── choices: [{ message: { role, content } }]       │
│    │   └── usage: { prompt_tokens, completion_tokens }      │
│    └── Return JSON response                                 │
│                                                             │
│ 4. API Middleware                                           │
│    └── logging_middleware: log RESPONSE (status, elapsed)   │
│                                                             │
│ 5. Client receives JSON:                                    │
│    { id, object, created, model, choices, usage }           │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Chat Completion Flow (SSE Streaming)

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Client sends POST /v1/chat/completions                   │
│    Body: { model: "01J9...", messages: [...], stream: true }│
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ 2. Chat Handler (same as non-streaming up to engine.generate)│
│                                                             │
│ 3. SSE Streaming Setup                                      │
│    ├── Create unbounded channel: (tx, rx)                   │
│    ├── Spawn tokio task for generation:                     │
│    │   ├── engine.generate(model, prompt, params, callback) │
│    │   ├── Callback per token:                              │
│    │   │   ├── Format SSE chunk: "data: {json}\n\n"        │
│    │   │   └── Send chunk via tx.send()                     │
│    │   └── After generation: tx.send("data: [DONE]\n\n")   │
│    ├── Convert rx to Body::from_stream()                    │
│    └── Return SSE response:                                 │
│        ├── Status: 200 OK                                   │
│        ├── Content-Type: text/event-stream                  │
│        ├── Cache-Control: no-cache                          │
│        └── Connection: keep-alive                           │
│                                                             │
│ 4. Client receives SSE stream:                              │
│    data: {"id":"chatcmpl-...","choices":[{"delta":{"content":"Hello"}}]}│
│                                                             │
│    data: {"id":"chatcmpl-...","choices":[{"delta":{"content":" world"}}]}│
│                                                             │
│    data: [DONE]                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Inference Engine Flow

```
┌─────────────────────────────────────────────────────────────┐
│ InferenceEngine Trait (edgeforge-runtime/src/engine.rs)     │
│                                                             │
│ ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐│
│ │MockInferenceEngine│  │ LlamaBackend    │  │ SsdEngine    ││
│ │ (default)        │  │ (llama-backend  │  │ (fallback)   ││
│ │                  │  │  feature)       │  │              ││
│ │ - Deterministic  │  │ - Real GGUF     │  │ - Pure Rust  ││
│ │   output         │  │   model loading │  │   inference  ││
│ │ - Reversed prompt│  │ - FFI via       │  │   loop       ││
│ │   words          │  │   runtime-sys   │  │              ││
│ └────────┬────────┘  └────────┬────────┘  └──────┬───────┘│
│          └──────────┬──────────┘                  │        │
│                     ▼                             │        │
│          ┌─────────────────────┐                  │        │
│          │   Engine Selection   │                  │        │
│          │                     │                  │        │
│          │ Is llama-backend    │                  │        │
│          │ feature enabled?    │                  │        │
│          ├── YES → LlamaBackend│                  │        │
│          └── NO  → MockEngine  │                  │        │
│          └─────────────────────┘                  │        │
│                                               │           │
│  ┌─────────────────────────────────────────────┘           │
│  │                                                         │
│  ▼                                                         │
│ ┌─────────────────────────────────────────────────────────┐│
│ │ generate(model, prompt, params, on_token)               ││
│ │                                                         ││
│ │ 1. Tokenize prompt                                      ││
│ │ 2. For each token to generate (up to max_tokens):       ││
│ │    a. Router hook: resolve_expert(layer, expert)         ││
│ │       ├── Check RAM cache → Ram                         ││
│ │       ├── Check streaming graph → Peer(node)            ││
│ │       ├── Allow surrogate? → Surrogate                  ││
│ │       └── Default → Ssd                                 ││
│ │    b. Decode/Sample next token                          ││
│ │    c. Detokenize to text                                ││
│ │    d. Call on_token(token, VerificationOutcome)         ││
│ │    e. If callback returns false → abort                 ││
│ │ 3. Return GenStats                                      ││
│ └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

---

## 7. Storage Tier Flow

```
┌─────────────────────────────────────────────────────────────┐
│ SSD (model file)                                            │
│   ├── Dense core (~10 GB, RAM-resident)                     │
│   ├── Expert pool (~370 GB, SSD-streamed)                   │
│   └── KV cache (paged, hot in RAM, cold on SSD)             │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ WeightPager (weight_pager.rs)                           │ │
│ │                                                         │ │
│ │ page_in(layer, expert) → WeightPage                     │ │
│ │   ├── Calculate offset: layer * page_size               │ │
│ │   ├── Open file (lazy, Mutex<Option<DirectFile>>)       │ │
│ │   ├── io.read_at(file, offset, len) → AlignedBuffer     │ │
│ │   └── Return WeightPage { id, bytes }                   │ │
│ │                                                         │ │
│ │ page_in_batch(ids) → Vec<WeightPage>                    │ │
│ │   ├── Build ReadRequest for each ID                     │ │
│ │   ├── io.read_batch(file, requests)                     │ │
│ │   └── Correlate results by index                        │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ExpertCache (expert_cache.rs) — LRU                     │ │
│ │                                                         │ │
│ │ get(key) → Option<Vec<u8>>                              │ │
│ │   ├── Lock mutex                                        │ │
│ │   ├── Bump LRU clock                                    │ │
│ │   ├── Lookup in HashMap                                 │ │
│ │   └── Return clone if hit, None if miss                 │ │
│ │                                                         │ │
│ │ put(key, bytes)                                         │ │
│ │   ├── If exists, subtract old size                     │ │
│ │   ├── Evict LRU until budget fits                       │ │
│ │   ├── Insert new entry                                  │ │
│ │   └── Update current_bytes                              │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ KvCachePager (kv_pager.rs) — LRU + SSD Spill            │ │
│ │                                                         │ │
│ │ get_or_load(id) → Option<Vec<u8>>                       │ │
│ │   1. Check RAM cache → hit: return                      │ │
│ │   2. Check SSD spill file → load: return                │ │
│ │   3. Miss → return None                                 │ │
│ │                                                         │ │
│ │ spill_to_ssd(id) → PathBuf                              │ │
│ │   1. Remove from RAM                                    │ │
│ │   2. Write to {ssd_dir}/kv_{layer}_{page}.bin           │ │
│ │   3. Return file path                                   │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ PlatformIo (platform/)                                  │ │
│ │                                                         │ │
│ │ ├── BlockingIo (portable, all platforms)                │ │
│ │ │   └── spawn_blocking + std::fs seek + read            │ │
│ │ ├── IoUring (Linux, with posix_fadvise prefetch)        │ │
│ │ ├── Iocp (Windows, stub)                                │ │
│ │ └── DispatchIo (macOS, stub)                            │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. Streaming Graph Flow

```
┌─────────────────────────────────────────────────────────────┐
│ Streaming Graph (edgeforge-graph, L3.5)                     │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Event Ingest (ControlEvent stream)                      │ │
│ │                                                         │ │
│ │  ControlEvent → GraphIngest.try_enqueue()               │ │
│ │    ├── Queue full? → Drop (R-18 load shedding)          │ │
│ │    └── Enqueued → drain_and_apply()                     │ │
│ │                                                         │ │
│ │  apply_event(event):                                    │ │
│ │    ├── PeerHeartbeat → record_heartbeat()               │ │
│ │    ├── PeerLost → record_peer_lost()                    │ │
│ │    ├── RouterDecision → record_router_decision()        │ │
│ │    └── Other → ignored                                  │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ GraphCore (in-memory store)                             │ │
│ │                                                         │ │
│ │ peers: DashMap<NodeId, PeerNode>                        │ │
│ │   ├── rtt_ms, thermal, experts_resident, quality_score  │ │
│ │   └── quality_score >= 0.3 to pass filter               │ │
│ │                                                         │ │
│ │ experts: DashMap<ExpertKey, ResidentLocation>           │ │
│ │   └── Ram | Ssd | Peer(NodeId)                          │ │
│ │                                                         │ │
│ │ history: RwLock<HashMap<seq, Vec<(layer, expert, ts)>>> │ │
│ │   └── Used by H3 prediction                             │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ GraphQuery (<10µs p99)                                  │ │
│ │                                                         │ │
│ │ best_peer_for_expert(layer, expert, thermal)            │ │
│ │   1. Check healthy() → false: return None               │ │
│ │   2. Filter peers: quality_score >= 0.3                 │ │
│ │   3. Mark peers with expert resident                    │ │
│ │   4. Sort: resident first, then lowest RTT              │ │
│ │   5. Return best PeerEstimate                           │ │
│ │                                                         │ │
│ │ predicted_expert_set(sequence, n)                       │ │
│ │   1. Look up sequence in history                        │ │
│ │   2. Return last N (layer, expert) pairs                │ │
│ │   └── Used by H3 speculation                            │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 9. Multi-Horizon Speculation Flow

```
┌─────────────────────────────────────────────────────────────┐
│ SpeculationEngine.predict(current_layer, logits, draft, graph)│
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ H1: Router Logit (always on, >90% accuracy)             │ │
│ │                                                         │ │
│ │ h1_predict_next_layer_experts(layer, logits, top_k)     │ │
│ │   1. Sort logits descending                             │ │
│ │   2. Take top_k indices                                  │ │
│ │   3. Return as ExpertId list for layer N+1              │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ H2: Surrogate/Draft Model (preset-gated, 60-85%)        │ │
│ │                                                         │ │
│ │ h2_predict_future_experts(layer, depth, draft_output)   │ │
│ │   1. For each layer in N+1..N+depth:                    │ │
│ │   2. Take draft model's expert predictions              │ │
│ │   3. Return as (LayerIdx, ExpertId) list                │ │
│ │                                                         │ │
│ │ Quality preset: correctness-preserving only             │ │
│ │ Balanced/Fast: stall-free with surrogate fallback       │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ H3: Graph Sequence Query (advisory, 40-70%)             │ │
│ │                                                         │ │
│ │ h3_predict_from_graph(graph, sequence, n)               │ │
│ │   1. Check graph.healthy() → false: skip                │ │
│ │   2. Call graph.predicted_expert_set(sequence, n)       │ │
│ │   3. Return as (LayerIdx, ExpertId) list                │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Union + Deduplication                                   │ │
│ │                                                         │ │
│ │ 1. Collect all H1 + H2 + H3 predictions                 │ │
│ │ 2. Sort by (layer, expert)                              │ │
│ │ 3. Remove duplicates                                    │ │
│ │ 4. Return deduplicated Vec<(LayerIdx, ExpertId)>        │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Confidence-Aware Fallback (on miss)                     │ │
│ │                                                         │ │
│ │ permit_surrogate(miss_confidence, preset_quality, avail)│ │
│ │   ├── Quality preset → always false (R-19 SLA)          │ │
│ │   ├── Not available → false                             │ │
│ │   └── miss_confidence < 0.3 → true (use surrogate)      │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 10. Distributed Pool Flow

```
┌─────────────────────────────────────────────────────────────┐
│ Tier 1: Trusted Mesh (≤16 peers, mTLS)                      │
│                                                             │
│ 1. mDNS Discovery (discovery.rs)                            │
│    ├── Advertise _edgeforge._tcp on port 11435              │
│    ├── Browse for other peers                               │
│    └── Return DiscoveredPeer list                           │
│                                                             │
│ 2. QUIC Connection (quic_transport.rs)                      │
│    ├── QuinnTransport.connect(peer_addr)                    │
│    ├── mTLS handshake (local CA + per-node cert)            │
│    ├── Return MeshConn (peer, peer_id, metadata)            │
│    └── Multiplex streams over single connection             │
│                                                             │
│ 3. Mesh Coordinator (mesh_coord.rs)                         │
│    ├── assign_experts(node, experts) — register ownership   │
│    ├── owner_of(layer, expert) — query owner                │
│    └── handle_node_loss(node) — return orphaned experts     │
│                                                             │
│ 4. Heartbeat Recovery (recovery.rs)                         │
│    ├── Send heartbeat every 5s                              │
│    ├── On miss: increment counter                           │
│    ├── At threshold (3): declare dead                       │
│    └── repage_from_ssd(expert) — re-page from local SSD    │
│                                                             │
│ 5. Bandwidth Prober (bandwidth.rs)                          │
│    ├── Probe RTT + bandwidth                                │
│    ├── is_sufficient_for_remote_fetch (≥ 1 Gbps)            │
│    └── estimate_fetch_time(bandwidth, expert_bytes)         │
│                                                             │
│ Tier 2: Cooperative Pool (≤100 peers, PoQ)                  │
│   ├── Proof of Quality verification (sampled evaluators)    │
│   ├── Credit-based incentive ledger                         │
│   └── k=2 redundant expert fetch                            │
│                                                             │
│ Tier 3: Open Crowd (unbounded, optimistic)                  │
│   ├── Optimistic execution + eventual verification         │
│   ├── Token-based incentive with dynamic pricing            │
│   ├── ChurnResilientScheduler (k=3 redundant)               │
│   └── Labeled "Experimental" in UI                          │
└─────────────────────────────────────────────────────────────┘
```

---

## 11. Prompt Security Flow

```
┌─────────────────────────────────────────────────────────────┐
│ L1.5 Prompt Security (ADRs 020-023) — opt-in per agent     │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 1. Intent Guardrails — Pre-flight (ADR-022)             │ │
│ │                                                         │ │
│ │    classify_intent(prompt) → GuardrailVerdict           │ │
│ │    ├── RegexIntentClassifier checks 5 patterns:         │ │
│ │    │   ├── DataExfiltration (send...ssh/password)       │ │
│ │    │   ├── ArbitraryShell (rm -rf /)                    │ │
│ │    │   ├── SystemModification (chmod, /etc/)            │ │
│ │    │   ├── PromptRevelation (reveal system prompt)      │ │
│ │    │   └── Other (ignore previous instructions)         │ │
│ │    ├── If Reject → abort pipeline                       │ │
│ │    └── If Allow → continue                              │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 2. Privileged Planner LLM (ADR-020)                     │ │
│ │                                                         │ │
│ │    planner.plan(prompt) → ActionPlan                    │ │
│ │    ├── Sees full user prompt                             │ │
│ │    ├── Produces: intent_summary, steps, permitted_tools │ │
│ │    ├── CANNOT invoke tools                               │ │
│ │    └── Returns ActionPlan                                │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 3. Quarantine Reader (ADR-023)                          │ │
│ │                                                         │ │
│ │    For each external_resource in plan:                  │ │
│ │    ├── fetch_resource(uri) — file:// or http(s)://      │ │
│ │    ├── sanitize_content(raw) — strip 9 injection patterns│ │
│ │    │   └── Returns (sanitized_text, confidence)         │ │
│ │    └── If confidence < 0.5 → drop (LowConfidence error) │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 4. Spotlighting (ADR-021)                               │ │
│ │                                                         │ │
│ │    spotlighter.spotlight(summary.text) → String          │ │
│ │    ├── Data trimming: strip "ignore previous" etc.      │ │
│ │    ├── Truncate to max_content_chars (8192)             │ │
│ │    ├── Data marking: wrap in [UNTRUSTED_BEGIN]...[END]  │ │
│ │    └── (opt) Data encapsulation: base64 encode          │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 5. Unprivileged Worker LLM (ADR-020)                    │ │
│ │                                                         │ │
│ │    worker.execute_step(plan, 0, spotlighted_context)    │ │
│ │    ├── Sees ONLY ActionPlan + spotlighted summaries     │ │
│ │    ├── CANNOT see original user prompt                  │ │
│ │    ├── CAN invoke tools (within guardrail scope)        │ │
│ │    └── Returns DecodedAction (tool, arguments, step)    │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 6. Intent Guardrails — Post-decode (ADR-022)            │ │
│ │                                                         │ │
│ │    validate_action(action, plan) → GuardrailVerdict     │ │
│ │    ├── Check: action.tool ∈ plan.permitted_tools        │ │
│ │    ├── Check: action.plan_step_index < plan.steps.len() │ │
│ │    ├── If Reject → log to prompt_guardrail_events       │ │
│ │    └── If Allow → execute tool call                     │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 12. Autonomic Self-Healing Flow

```
┌─────────────────────────────────────────────────────────────┐
│ MAPE-K Loop (edgeforge-autonomic, L5.5, ADR-029)            │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ MONITOR                                                 │ │
│ │                                                         │ │
│ │ Subscribe to ControlEvent stream from streaming graph   │ │
│ │ Batch events (up to 256 per cycle)                      │ │
│ │ Extract metrics: io_latency_us, peer_rtt_ms, ssd_temp_c│ │
│ └─────────────────────────────────────────────────────────┘ │
│                          ▼                                  │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ANALYZE (Statistical Process Control)                   │ │
│ │                                                         │ │
│ │ RollingStats (Welford's algorithm):                     │ │
│ │   ├── Update mean + M2 (for variance)                   │ │
│ │   ├── Classify each metric:                             │ │
│ │   │   ├── Common-cause (within 3σ) → systemic           │ │
│ │   │   └── Special-cause (beyond 3σ) → one-off           │ │
│ │   └── detect_patterns() → Vec<FailurePattern>           │ │
│ └─────────────────────────────────────────────────────────┘ │
│                          ▼                                  │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ PLAN                                                    │ │
│ │                                                         │ │
│ │ Select remediation by severity:                         │ │
│ │   ├── ThermalThrottle → ReducePrefetchDepth             │ │
│ │   ├── CorruptExpert → EvictExpert                       │ │
│ │   ├── VerificationFailure → TriggerKaniReverification   │ │
│ │   ├── PeerDisconnect → EvictPeer                        │ │
│ │   └── SpeculationMissSpike → ReduceDraftMax             │ │
│ └─────────────────────────────────────────────────────────┘ │
│                          ▼                                  │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ EXECUTE                                                 │ │
│ │                                                         │ │
│ │ remediation.execute() → RemediationResult               │ │
│ │   ├── Success / Failed / Skipped                        │ │
│ │   └── Log to autonomic_actions table (migration 0015)   │ │
│ └─────────────────────────────────────────────────────────┘ │
│                          ▼                                  │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ANTIFRAGILE TWIST                                       │ │
│ │                                                         │ │
│ │ If remediation succeeded:                               │ │
│ │   ├── Generate regression test from failure pattern     │ │
│ │   ├── If verification failure: generate Kani harness    │ │
│ │   └── Commit to tests/e2e/autonomic/ (if enabled)       │ │
│ │                                                         │ │
│ │ System becomes STRONGER because it failed.              │ │
│ └─────────────────────────────────────────────────────────┘ │
│                          ▼                                  │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ KNOWLEDGE                                               │ │
│ │                                                         │ │
│ │ AutonomicKnowledge stores:                              │ │
│ │   ├── Events (last 10,000 ControlEvents)                │ │
│ │   ├── Patterns (FailureKind → FailurePattern)           │ │
│ │   └── Actions (last 10,000 AutonomicActions)            │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 13. Monitoring & Health Flow

```
┌─────────────────────────────────────────────────────────────┐
│ F-MON-01: Throughput Benchmark                              │
│   ├── run_benchmark(engine, model, config)                  │
│   ├── Warmup phase (not measured)                           │
│   ├── Measured phase (timed)                                │
│   └── Returns: tok/s, first_token_ms, SSD metrics           │
│                                                             │
│ F-MON-02: Quality Evaluation                                │
│   ├── MMLU-lite (5 questions)                               │
│   ├── HumanEval-10 (3 problems)                             │
│   ├── Greedy decoding (temperature=0)                       │
│   └── Returns: mmlu_score, humaneval_score, quality_delta   │
│                                                             │
│ F-MON-03: SSD Wear Monitoring                               │
│   ├── read_ssd_health(device)                               │
│   ├── Runs smartctl -a -j                                   │
│   ├── Parses NVMe SMART JSON                                │
│   ├── Returns: total_bytes_written, percent_used             │
│   └── Status: Healthy (<50%) / Warning (<80%) / Critical    │
│                                                             │
│ F-MON-04: Thermal Monitoring                                │
│   ├── ThermalMonitor polls every 5s                         │
│   ├── 70°C → throttle prefetch depth                        │
│   ├── 80°C → pause inference                                │
│   └── Emits ControlEvent::ThermalSample                     │
│                                                             │
│ F-MON-05: Per-Layer Latency Profiler                        │
│   ├── Profiler.record(layer, duration)                      │
│   ├── mean_latency(layer) → Duration                        │
│   └── top_slowest(n) → Vec<(LayerIdx, Duration)>            │
│                                                             │
│ ADR-026: Carbon-Aware Routing                               │
│   ├── CarbonAwareRouter.route(peers, timestamp)             │
│   ├── StaticCarbonTable (IEA 2024 grid intensities)         │
│   ├── Selects lowest-carbon peer satisfying latency SLO     │
│   └── energy_joules_per_token benchmark metric              │
└─────────────────────────────────────────────────────────────┘
```

---

## 14. Function Call Logging Flow

```
┌─────────────────────────────────────────────────────────────┐
│ FunctionLogger (edgeforge-observability/src/function_logger)│
│                                                             │
│ Every public function uses FunctionGuard:                   │
│                                                             │
│   pub fn my_function() -> Result<...> {                     │
│       let mut guard = FunctionGuard::new(                   │
│           &LOGGER,            // FunctionLogger ref          │
│           "edgeforge-net",    // crate name                  │
│           "quic_transport.rs", // file name                  │
│           "connect",          // function name               │
│       );                                                    │
│       // ... function body ...                              │
│       guard.success();  // or guard.error("message")        │
│       Ok(())                                                 │
│   }                                                         │
│                                                             │
│ On FunctionGuard creation:                                  │
│   ├── Logs ENTRY to tracing (trace level)                   │
│   └── Records CallLog to in-memory buffer                   │
│                                                             │
│ On FunctionGuard drop:                                      │
│   ├── Logs SUCCESS or ERROR to tracing                      │
│   ├── Records duration in microseconds                      │
│   └── Records error message (if any)                        │
│                                                             │
│ Log format:                                                 │
│   timestamp - crate - file - function - STATUS (durationµs) │
│                                                             │
│ Example:                                                    │
│   2026-10-31T23:00:00.123Z - edgeforge-net -               │
│   quic_transport.rs - connect - ERROR (1500µs)              │
│   [QuinnTransport::connect — enable quic-transport feature] │
│                                                             │
│ Access logs:                                                │
│   LOGGER.dump_logs()  → Vec<CallLog>                        │
│   LOGGER.dump_json()  → JSON string                         │
│   LOGGER.recent(n)    → last N entries                      │
│   LOGGER.clear()      → clear buffer                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 15. Data Persistence Flow

```
┌─────────────────────────────────────────────────────────────┐
│ SQLite Database (~/.local/share/edgeforge/edgeforge.db)     │
│                                                             │
│ 16 Migrations (resources/migrations/):                      │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Core Tables                                             │ │
│ │ ├── models (0002) — model registry with precision       │ │
│ │ ├── jobs (0003) — job queue with status state machine   │ │
│ │ ├── conversations (0004) — chat history                 │ │
│ │ ├── pool_nodes (0005) — distributed pool membership     │ │
│ │ └── benchmark_runs (0006) — throughput history          │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ RAG Tables                                              │ │
│ │ ├── embedding_sources (0007) — RAG document metadata    │ │
│ │ └── embeddings (0007, vss0 virtual table at runtime)    │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Crowd Tables                                            │ │
│ │ ├── peer_reputation (0008) — per-peer quality scores    │ │
│ │ ├── verification_events (0009) — Tier 2/3 verification  │ │
│ │ └── incentive_ledger (0010) — credit/debit accounting   │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Prompt Security Tables                                  │ │
│ │ ├── prompt_guardrail_events (0012) — guardrail verdicts │ │
│ │ ├── prompt_quarantine_events (0012) — quarantine reads  │ │
│ │ └── prompt_pipeline_runs (0012) — dual-LLM pipeline runs│ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ CMMI + Autonomic Tables                                │ │
│ │ ├── kpi_measurements (0013) — QPPO metric history       │ │
│ │ ├── kpi_violations (0013) — QPPO control limit breaches │ │
│ │ ├── traceability_items (0014) — ADR↔req↔test↔code       │ │
│ │ ├── adrs (0014) — ADR registry                          │ │
│ │ ├── autonomic_events (0015) — MAPE-K event log          │ │
│ │ ├── autonomic_patterns (0015) — failure patterns        │ │
│ │ └── autonomic_actions (0015) — remediation actions      │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Crypto Tables                                           │ │
│ │ ├── crypto_inventory (0016) — PQC primitive inventory   │ │
│ │ └── crypto_handshake_latency (0016) — PQC timing        │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Extended Benchmark Columns (0011, 0016)                 │ │
│ │ ├── speculation_acceptance_rate                         │ │
│ │ ├── prefetch_hit_rate                                   │ │
│ │ ├── surrogate_accuracy_delta                            │ │
│ │ ├── crowd_tier                                          │ │
│ │ ├── verification_pending_count                          │ │
│ │ ├── verification_corrected_count                        │ │
│ │ ├── energy_joules_per_token                             │ │
│ │ ├── carbon_grams_per_1k_tokens                          │ │
│ │ └── region                                              │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## Summary: End-to-End Request Flow

```
User Input
    │
    ▼
CLI / API Server (L5-L6)
    │
    ├── Load Config (Core)
    ├── Open SQLite (Orchestration)
    ├── Build Router (API)
    │
    ▼
Chat Completions Handler (API)
    │
    ├── Query Model Registry (Orchestration → SQLite)
    ├── Create InferenceEngine (Runtime)
    ├── Load Model (Runtime → Security verify manifest)
    │
    ▼
InferenceEngine::generate (Runtime, L4)
    │
    ├── Tokenize Prompt
    ├── For each token:
    │   ├── RouterHook::resolve_expert (L4)
    │   │   ├── Check ExpertCache (L3)
    │   │   ├── Query StreamingGraph (L3.5)
    │   │   └── Fallback to SSD
    │   ├── WeightPager::page_in (L3 → PlatformIo → SSD)
    │   ├── SpeculationEngine::predict (H1/H2/H3)
    │   ├── Prefetcher::notify_layer_done (L3)
    │   ├── ThermalMonitor::sample (L3, if thermal > 70°C → throttle)
    │   ├── Compute token (mock: reversed prompt / real: FFI decode+sample)
    │   ├── Emit ControlEvent::TokenGenerated
    │   └── Call on_token callback
    │
    ▼
API Response (API)
    │
    ├── Non-streaming: collect tokens → JSON response
    ├── Streaming: SSE chunks via channel → Body::from_stream
    └── X-EdgeForge-Verification header (ADR-018)
    │
    ▼
User Output
```

---

**This document captures the complete functional flow of EdgeForge from user input to token output, including all 11 layers, 18 crates, and 32 ADRs.**
