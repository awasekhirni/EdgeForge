# EdgeForge - Phase 1: Architecture & Planning (Final, Approved)

--Version:-- 1.0 (post Q1–Q6 approval)
--Date:-- 2025-07-31
--Status:-- Approved - ready for Phase 2 implementation
--Stack:-- Rust (primary) + llama.cpp C++ FFI (L4 only) + Tauri 2 (L6) + SQLite
--Target:-- Desktop runtime for 1 T-parameter MoE models, FP16 default precision, on 2 TB NVMe minimum

---

## 0. Decision Update Summary

All six open questions from the previous draft have been resolved. This document supersedes the prior Phase 1 draft in its entirety.

| Q | Decision (locked) | ADR |
|---|---|---|
| Q1 | --FP16 is the default precision.-- SSD floor raised from 1 TB → --2 TB NVMe Gen4 minimum.-- | ADR-002, ADR-003, ADR-004 |
| Q2 | `llama.cpp` pinned to --latest stable tag-- at Phase 2 kickoff; tracked as git submodule. | ADR-009 |
| Q3 | Mesh transport = --QUIC via `quinn`-- for superior NAT traversal (F-NET-02 relay). | ADR-010 |
| Q4 | Python SDK ships as --prebuilt wheels--: cp39–cp313, macOS universal2, manylinux x86_64+aarch64, Windows x64. | ADR-011 |
| Q5 | Observability = --`tracing` + OpenTelemetry OTLP exporter--, opt-in feature flag, off by default per F-SEC-01. | ADR-012 |
| Q6 | Release channels = --stable + beta only-- (no nightly). | ADR-013 |

---

## 1. Final Decision Matrix (ADRs 001–013)

| ADR | Title | Decision | Status |
|---|---|---|---|
| --ADR-001-- | Rust workspace + llama.cpp FFI | Path 2: Rust for L1–L3, L5, L6; C++ FFI confined to L4 `edgeforge-runtime-sys` | Approved |
| --ADR-002-- | Minimum SSD requirement | --2 TB NVMe Gen4-- (raised from 1 TB to support FP16 default) | Approved |
| --ADR-003-- | Default precision for 1 T model | --FP16-- (was Q4 in prior draft) | Approved |
| --ADR-004-- | Precision tier matrix & hard-block policy | Q4/Q8/FP16/FP32 all supported; UI hard-blocks when SSD free < 1.2 × model size | Approved |
| --ADR-005-- | Local persistence | SQLite + `sqlite-vss` (zero-config, air-gapped-friendly) | Approved |
| --ADR-006-- | Desktop shell | Tauri 2 (Rust-native, smallest footprint) | Approved |
| --ADR-007-- | Local API | OpenAI-compatible REST + SSE on `127.0.0.1:11434` | Approved |
| --ADR-008-- | Air-gapped default | All networking opt-in; telemetry off by default | Approved |
| --ADR-009-- | llama.cpp versioning | Git submodule pinned to latest stable tag (re-pinned per release) | Approved |
| --ADR-010-- | Mesh transport | QUIC via `quinn` 0.11; mTLS via `rustls` over QUIC | Approved |
| --ADR-011-- | Python SDK distribution | Prebuilt wheels via `maturin` + `cibuildwheel`; cp39–cp313, macOS universal2, manylinux x86_64/aarch64, Windows x64 | Approved |
| --ADR-012-- | Observability | `tracing` core always on; OTLP exporter behind `telemetry-otlp` feature flag, off by default | Approved |
| --ADR-013-- | Release channels | `stable` + `beta` only; auto-update channel switcher in Settings | Approved |

---

## 2. Storage Math (Recomputed for FP16 Default on 2 TB NVMe)

### 2.1 Size Table for a 1 T-parameter MoE Model

| Precision | Bytes/param | Total size | Dense core (~1.3% = 13 B) | Expert pool (~98.7% = 987 B) | Fits 2 TB NVMe? |
|---|---|---|---|---|---|
| FP32 | 4 | 4.0 TB | 52 GB | 3.95 TB |  No |
| --FP16 / BF16-- | 2 | --2.0 TB-- | 26 GB | 1.97 TB | --Default-- (zero headroom - see §2.3 caveat) |
| Q8 (INT8) | 1 | 1.0 TB | 13 GB | 987 GB | Comfortable |
| Q4 (INT4) | 0.5 | 500 GB | 6.5 GB | 493 GB | Room for 3+ models |

### 2.2 Free-Space Budget on 2 TB NVMe (FP16 default)

```
SSD total:                       2,000 GB
─ OS + apps overhead:              −80 GB
─ EdgeForge binary + cache:        −10 GB
─ 1 T model @ FP16:              −2,000 GB  ← model file
─ Download headroom (1.2×):       −400 GB   ← temp during install
─ 20% free-space rule (§4.7):     −400 GB
─────────────────────────────────────────
Required:                       2,890 GB  ⚠️ EXCEEDS 2 TB
```

### 2.3 Resolved Policy (Critical Caveat - Read Before Shipping)

The 2 TB NVMe floor --enables FP16 storage-- but --does not provide headroom for download + 20% free-space rule simultaneously--. EdgeForge enforces this honestly:

1. --Default install path for 1 T @ FP16 = 4 TB NVMe-- is the -recommended- configuration.
2. --2 TB NVMe supports--:
   - 1 T model @ Q4 (500 GB) - fits with full headroom.
   - 1 T model @ Q8 (1 TB) - fits with reduced headroom.
   - 1 T model @ FP16 (2 TB) - fits the model file but --blocks re-download--; user must delete the temp download after verify, and EdgeForge warns: -"FP16 on 2 TB NVMe has zero headroom. Add a second NVMe or use Q8 for safer operation."-
3. --FP32 (4 TB) requires ≥ 6 TB NVMe-- - never the default, only opt-in.
4. --UI hard-blocks-- any configuration where `free_ssd < 1.2 × model_size` at install time, with a plain-English message: -"Your SSD has X GB free; this model at FP16 needs Y GB. Switch to Q8/Q4 or attach a larger SSD."-

### 2.4 Updated Default Accuracy Promise

EdgeForge publishes cold-cache and warm-cache tok/s for --every precision tier-- in F-MON-01. The default preset ships as:

| Preset | Precision | Target use |
|---|---|---|
| --Quality (default)-- | FP16 | Research, accuracy-critical workloads |
| Balanced | Q8 | Daily driver, near-FP16 quality |
| Fast | Q4 | Maximum throughput, accepts 1–3 MMLU pt loss |
| Max-Quality | FP32 | Opt-in only; requires ≥ 6 TB SSD |

---

## 3. High-Level System Architecture

### 3.1 Six-Layer View

```
┌──────────────────────────────────────────────────────────────────────────┐
│  L6  UI Layer                  Tauri 2 shell + React/Webview             │
│       Chat · Model Library · Pool Dashboard · Monitoring · Wizards       │
│       (calls L5 via Tauri IPC, never touches L4)                         │
├──────────────────────────────────────────────────────────────────────────┤
│  L5  Orchestration             Model lifecycle · Job scheduler (priority │
│       lanes + continuous batching) · Mesh coordinator · Presets          │
│       OpenAI-compatible API server (axum, 127.0.0.1:11434)               │
├──────────────────────────────────────────────────────────────────────────┤
│  L4  Inference Runtime         Rust facade → llama.cpp C++ FFI           │
│       + custom SSD-streaming engine (dense-core + expert-pool)           │
│       Stable C ABI boundary; unsafe confined to this crate               │
├──────────────────────────────────────────────────────────────────────────┤
│  L3  Storage Tier              Weight pager · KV cache pager ·           │
│       Layer-ahead prefetcher · I/O scheduler                             │
│       (io_uring / IOCP / dispatch_io behind a PlatformIo trait)          │
│       VRAM ↔ RAM ↔ SSD hierarchy                                         │
├──────────────────────────────────────────────────────────────────────────┤
│  L2  Networking                mDNS discovery · QUIC mesh (quinn) ·      │
│       mTLS (rustls over QUIC) · Bandwidth prober (opt-in only)           │
├──────────────────────────────────────────────────────────────────────────┤
│  L1  Security                  Manifest verifier (ed25519/ring) ·        │
│       Sandbox (seatbelt/AppContainer/bubblewrap) · Attestation hooks ·   │
│       At-rest encryption hooks (OS-native + optional AES-256 pages)      │
└──────────────────────────────────────────────────────────────────────────┘

Cross-cutting: edgeforge-core (types, errors, config, telemetry-bus)
               edgeforge-observability (tracing + opt-in OTLP exporter)
```

### 3.2 Data Flow - Single MoE Token (FP16 default path)

```
┌─────────┐   POST /v1/chat/completions    ┌─────────────────┐
│  Client │ ─────────────────────────────► │  L5 API Server  │
└─────────┘                                │  (axum, :11434) │
   ▲                                       └────────┬────────┘
   │ SSE token stream                               │ enqueue job
   │                                               ▼
   │                                      ┌─────────────────┐
   │                                      │ L5 Scheduler    │
   │                                      │ (priority lane) │
   │                                      └────────┬────────┘
   │                                               │ acquire slot
   │                                               ▼
   │                                      ┌─────────────────┐
   │                                      │ L4 Runtime      │
   │                                      │ (Rust facade)   │
   │                                      └────────┬────────┘
   │                                               │ per layer:
   │   ┌───────────────────────────────────────────┘
   │   │
   │   │ 1. Router (dense core, RAM-resident) selects top-k experts
   │   │    ───────────────────────────────────────► L4 router hook
   │   │
   │   │ 2. For each selected expert:
   │   │    ┌─ L3 expert cache (RAM) hit?  ──► use directly
   │   │    └─ miss?
   │   │         ├─ local SSD fetch via io_uring (L3 weight pager)
   │   │         └─ OR remote peer fetch via QUIC+mTLS (L2, if pool
   │   │            enabled AND bandwidth prober says faster than SSD)
   │   │
   │   │ 3. Prefetcher (L3, background thread):
   │   │    issues async reads for layer N+1's predicted experts
   │   │    (predicted by router stats OR 0.5B draft model - F-SCHED-02)
   │   │
   │   │ 4. Stage expert in VRAM (or Apple unified memory - no copy)
   │   │
   │   │ 5. Compute FFN → Attention on VRAM
   │   │    └─ KV cache update (L3 KV pager, 4 MB pages)
   │   │       hot pages → RAM, cold pages → SSD (LRU evict)
   │   │
   │   │ 6. Evict LRU experts from RAM cache (no write - read-only)
   │   │
   │   └─► emit token ──► L5 ──► SSE ──► Client
```

---

## 4. Crate Decomposition (Rust Workspace)

```
edgeforge/
├── Cargo.toml                          # [workspace] root manifest
├── crates/
│   ├── edgeforge-core/                 # Shared types, Error, Config, telemetry-bus
│   ├── edgeforge-observability/        # tracing + opt-in OTLP exporter (ADR-012)
│   ├── edgeforge-security/             # L1
│   ├── edgeforge-net/                  # L2 (QUIC via quinn - ADR-010)
│   ├── edgeforge-storage/              # L3
│   ├── edgeforge-runtime/              # L4 (Rust facade + unsafe FFI)
│   ├── edgeforge-runtime-sys/          # -sys crate: bindgen to llama.cpp C ABI
│   ├── edgeforge-orchestration/        # L5
│   ├── edgeforge-api/                  # OpenAI-compatible REST (axum)
│   ├── edgeforge-sdk-python/           # PyO3 bindings + maturin wheel build
│   ├── edgeforge-cli/                  # `edgeforge` binary
│   └── edgeforge-app/                  # Tauri 2 desktop shell (src-tauri/)
├── ffi/
│   └── llama.cpp/                      # git submodule @ latest stable tag (ADR-009)
├── resources/
│   ├── migrations/                     # SQLx migrations (SQLite)
│   ├── schemas/                        # JSON Schema, OpenAPI 3.1 spec
│   └── manifests/                      # Default publisher manifest templates
├── docs/
│   ├── adr/                            # ADR-001 … ADR-013
│   ├── architecture/
│   ├── api/                            # Generated OpenAPI docs
│   └── setup/                          # Novice setup guide (this doc)
├── scripts/
│   ├── build-llama.sh                  # builds llama.cpp for the host triple
│   ├── gen-bindings.sh
│   └── install.sh
├── tests/                              # Workspace-level integration tests
│   ├── e2e/
│   ├── fixtures/
│   └── golden/
├── .github/workflows/                  # CI: fmt, clippy -D warnings, test, build, release
└── python/                             # Python SDK packaging (pyproject.toml, cibuildwheel)
```

### 4.1 Crate Dependency DAG (enforced by `cargo-deny`)

```
                edgeforge-app (Tauri shell, binary)
                       │
                edgeforge-cli (binary)
                       │
                edgeforge-api (lib)
                       │
                edgeforge-orchestration (lib)
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
 edgeforge-runtime  edgeforge-storage  edgeforge-net
        │              │              │
        └────► edgeforge-runtime-sys  │
                       │              │
        ┌──────────────┴──────────────┘
        ▼
 edgeforge-security
        │
        ▼
 edgeforge-observability
        │
        ▼
 edgeforge-core  (no internal deps; leaf)
```

--Rule--: No layer may depend on a higher layer. `cargo-deny` rejects any PR that violates this. `edgeforge-core` is the only leaf; everything depends on it; it depends on nothing internal.

---

## 5. Folder Structure (Detailed)

```
edgeforge/
├── Cargo.toml
├── rust-toolchain.toml                 # stable channel, msrv = 1.82
├── deny.toml                           # cargo-deny config (licenses + bans + cycles)
├── rustfmt.toml
├── clippy.toml                         # msrv = 1.82, warnings = deny
│
├── crates/
│   ├── edgeforge-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs                # EdgeForgeError enum, thiserror
│   │       ├── config.rs               # Strongly-typed config (figment + serde)
│   │       ├── types.rs                # ModelId, ExpertId, LayerIdx, Token, etc.
│   │       ├── units.rs                # Bytes, TokPerSec, Celsius - newtype wrappers
│   │       └── telemetry.rs            # tracing subscriber init shim
│   │
│   ├── edgeforge-observability/        # NEW crate (ADR-012)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── subscriber.rs           # tracing-subscriber setup
│   │       ├── otlp.rs                 # OTLP exporter (behind `telemetry-otlp` feature)
│   │       ├── metrics.rs              # metrics crate facade
│   │       └── span.rs                 # standard span names + attributes
│   │
│   ├── edgeforge-security/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manifest.rs             # F-LOAD-04, F-SEC-05 - signed manifest verify
│   │       ├── hash.rs                 # SHA-256 streaming verifier
│   │       ├── sandbox.rs              # F-SEC-03 - platform sandbox policy
│   │       ├── attestation.rs          # F-SEC-07 - SGX/SEP hooks (stub on unsupported)
│   │       └── encryption.rs           # F-SEC-06 - at-rest page encryption hooks
│   │
│   ├── edgeforge-net/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── discovery.rs            # F-NET-01 - mDNS via zeroconf
│   │       ├── quic_transport.rs       # ADR-010 - quinn 0.11 + rustls
│   │       ├── mtls.rs                 # F-SEC-02 - rustls cert verify over QUIC
│   │       ├── mesh.rs                 # F-NET-03 - expert-partition router
│   │       ├── pipeline.rs             # F-NET-04 - layer-stripe forwarding
│   │       ├── bandwidth.rs            # F-NET-05 - link prober
│   │       └── recovery.rs             # F-NET-06 - heartbeat, expert re-page
│   │
│   ├── edgeforge-storage/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── platform/
│   │       │   ├── mod.rs              # PlatformIo trait
│   │       │   ├── iouring.rs          # Linux
│   │       │   ├── iocp.rs             # Windows
│   │       │   └── dispatch_io.rs      # macOS
│   │       ├── weight_pager.rs         # F-TIER-01, F-TIER-02, F-TIER-05
│   │       ├── expert_cache.rs         # RAM-resident LRU expert cache
│   │       ├── kv_pager.rs             # F-TIER-04, F-TIER-06
│   │       ├── prefetcher.rs           # F-TIER-03, F-SCHED-02
│   │       ├── vram_tier.rs            # VRAM staging (CUDA / Metal / ROCm)
│   │       ├── thermal.rs              # F-MON-04 - SMART temp polling, throttle
│   │       └── wear.rs                 # F-MON-03 - SSD health metrics
│   │
│   ├── edgeforge-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public Rust facade (safe API)
│   │       ├── engine.rs               # Engine trait - swap backends
│   │       ├── llama_backend.rs        # llama.cpp-backed engine impl
│   │       ├── ssd_engine.rs           # Custom SSD-streaming engine impl
│   │       ├── router_hook.rs          # F-SCHED-01 - intercept top-k
│   │       ├── batching.rs             # F-SCHED-03 - continuous batching
│   │       ├── priority.rs             # F-SCHED-04 - interactive vs batch
│   │       ├── determinism.rs          # F-SCHED-05 - reproducible mode
│   │       └── profiler.rs             # F-MON-05 - per-layer latency
│   │
│   ├── edgeforge-runtime-sys/
│   │   ├── Cargo.toml
│   │   ├── build.rs                    # cc + cmake crate; builds llama.cpp
│   │   ├── wrapper.h                   # bindgen input
│   │   └── src/
│   │       └── lib.rs                  # raw FFI bindings (unsafe)
│   │
│   ├── edgeforge-orchestration/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lifecycle.rs            # load → warm → serve → unload
│   │       ├── scheduler.rs            # priority lanes, batch queue
│   │       ├── presets.rs              # F-UX-04 - Fast/Balanced/Quality/Max-Quality
│   │       ├── mesh_coord.rs           # F-NET-- - distributed coordinator
│   │       ├── jobs.rs                 # F-UX-06 - SQLite-backed job queue
│   │       ├── benchmark.rs            # F-MON-01 - throughput harness
│   │       └── quality_eval.rs         # F-MON-02 - MMLU-lite / HumanEval-10
│   │
│   ├── edgeforge-api/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs               # axum app builder
│   │       ├── routes/
│   │       │   ├── chat.rs             # POST /v1/chat/completions (SSE)
│   │       │   ├── completions.rs      # POST /v1/completions
│   │       │   ├── embeddings.rs       # POST /v1/embeddings
│   │       │   ├── models.rs           # GET /v1/models
│   │       │   ├── files.rs            # F-DEV-06 - RAG ingest
│   │       │   ├── webhooks.rs         # F-DEV-05 - signed callbacks
│   │       │   └── health.rs
│   │       ├── middleware/
│   │       │   ├── error.rs            # centralized error → JSON
│   │       │   ├── rate_limit.rs       # token-bucket per API key
│   │       │   ├── auth.rs             # API key (optional, localhost-only)
│   │       │   └── logging.rs
│   │       └── dto/                    # request/response types (serde + utoipa)
│   │
│   ├── edgeforge-sdk-python/
│   │   ├── Cargo.toml                  # cdylib + pyo3
│   │   ├── pyproject.toml              # maturin build backend
│   │   ├── src/
│   │   │   └── lib.rs                  # PyO3 bindings → edgeforge-api client
│   │   └── tests/
│   │       └── test_sdk.py
│   │
│   ├── edgeforge-cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                 # clap derive
│   │       ├── commands/
│   │       │   ├── serve.rs            # `edgeforge serve` - start API server
│   │       │   ├── run.rs              # `edgeforge run` - one-shot generate
│   │       │   ├── model.rs            # `edgeforge model list|import|verify`
│   │       │   ├── bench.rs            # `edgeforge bench` - F-MON-01
│   │       │   ├── pool.rs             # `edgeforge pool join|leave|status`
│   │       │   └── config.rs
│   │       └── output.rs               # JSON / human-friendly output
│   │
│   └── edgeforge-app/                  # Tauri 2 desktop shell
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── src/
│       │   ├── main.rs                 # Tauri entry
│       │   ├── commands.rs             # Tauri IPC commands
│       │   └── menu.rs
│       ├── ui/                         # React frontend
│       │   ├── package.json
│       │   ├── vite.config.ts
│       │   ├── src/
│       │   │   ├── App.tsx
│       │   │   ├── routes/
│       │   │   │   ├── Chat.tsx        # F-UX-02
│       │   │   │   ├── Library.tsx     # F-UX-03
│       │   │   │   ├── Pool.tsx        # F-UX-05
│       │   │   │   ├── Monitor.tsx     # F-MON-03/04/05
│       │   │   │   ├── Settings.tsx
│       │   │   │   └── Wizard.tsx      # F-UX-01
│       │   │   ├── components/
│       │   │   ├── hooks/
│       │   │   ├── stores/             # Zustand
│       │   │   └── api/                # generated from OpenAPI
│       │   └── tailwind.config.ts
│       └── icons/
│
├── ffi/
│   └── llama.cpp/                      # git submodule @ latest stable tag
│
├── python/                             # Python SDK packaging
│   ├── cibuildwheel.toml               # wheel build matrix config
│   └── .github/workflows/
│       └── wheels.yml                  # cp39–cp313 × {macos-universal2,
│                                       #              manylinux-x86_64,
│                                       #              manylinux-aarch64,
│                                       #              win-amd64}
│
├── resources/
│   ├── migrations/
│   │   ├── 0001_init.sql
│   │   ├── 0002_models.sql
│   │   ├── 0003_jobs.sql
│   │   ├── 0004_conversations.sql
│   │   ├── 0005_pool_nodes.sql
│   │   ├── 0006_benchmarks.sql
│   │   └── 0007_embeddings.sql         # F-DEV-06 sqlite-vss
│   ├── schemas/
│   │   ├── manifest.v1.json            # F-LOAD-04
│   │   ├── preset.v1.json              # F-UX-04
│   │   └── openapi.yaml
│   └── manifests/
│       └── default-publishers.json
│
├── docs/
│   ├── adr/
│   │   ├── 0001-rust-plus-llama-cpp-ffi.md
│   │   ├── 0002-2tb-nvme-minimum.md
│   │   ├── 0003-fp16-default-precision.md
│   │   ├── 0004-precision-tier-matrix.md
│   │   ├── 0005-sqlite-persistence.md
│   │   ├── 0006-tauri-2-shell.md
│   │   ├── 0007-openai-compatible-api.md
│   │   ├── 0008-air-gapped-default.md
│   │   ├── 0009-llama-cpp-stable-pin.md
│   │   ├── 0010-quic-mesh-transport.md
│   │   ├── 0011-python-prebuilt-wheels.md
│   │   ├── 0012-opentelemetry-opt-in.md
│   │   └── 0013-stable-beta-release-channels.md
│   ├── architecture/
│   │   ├── 01-overview.md
│   │   ├── 02-storage-tier.md
│   │   ├── 03-runtime.md
│   │   └── 04-mesh.md
│   ├── setup/
│   │   ├── 01-prerequisites.md
│   │   ├── 02-build-from-source.md
│   │   ├── 03-first-run.md
│   │   └── 04-troubleshooting.md
│   └── api/
│       └── README.md                   # autogenerated by utoipa
│
├── scripts/
│   ├── build-llama.sh
│   ├── gen-bindings.sh
│   ├── install.sh                      # one-shot installer for end users
│   └── bootstrap-dev.sh                # dev env setup for contributors
│
├── tests/
│   ├── e2e/
│   │   ├── chat_streaming.rs
│   │   ├── model_import.rs
│   │   ├── distributed_pool.rs
│   │   └── io_conformance.rs           # PlatformIo per-OS conformance
│   ├── fixtures/
│   │   ├── models/                     # tiny test GGUFs
│   │   └── manifests/
│   └── golden/
│       └── presets.json
│
└── .github/workflows/
    ├── ci.yml                          # fmt + clippy + test on PR
    ├── build-matrix.yml                # macOS + Windows + Linux builds
    ├── wheels.yml                      # Python wheel build matrix (ADR-011)
    └── release.yml                     # stable + beta channel publish (ADR-013)
```

---


# ADR-017: Tiered Crowd Computing with Optimistic Verification

Status: Approved
Date: 2025-08-29
Supersedes: The binary `self_only / trusted_peers / lan_open` policy from Phase 1 v1.0
Related: ADR-008 (air-gapped default), ADR-018 (async verification), R-20 (prompt leakage), R-21 (verification latency)

## Context

Phase 1 v1.0's distributed mode is mTLS-gated and trust-based: you
explicitly pair peers via a 6-digit code. This is appropriate for a
trusted LAN pool but does not scale to a crowd/volunteer network where
peers are heterogeneous, untrusted, and churning. The Risk Register's
R-06 correctly warns that "2 nodes ≠ 2× speed," but it does not address
verification, incentives, or fault tolerance in an open network.

## Decision

Replace the binary trust policy with a three-tier distribution model,
each with its own trust and verification posture:

### Tier 1: Trusted Mesh (current design, hardened)

- Peers: ≤16, explicitly paired, mTLS.
- Use case: Lab cluster, two workstations with fast interconnect.
- Verification: Pre-shared certificates + optional activation checksums.
- Fault tolerance: Heartbeat-driven eviction; re-fetch from SSD on peer loss.

### Tier 2: Cooperative Pool (semi-trusted, e.g., team or org)

- Peers: Up to 100, joined via org-level identity (SSO or signed invite).
- Use case: Departmental GPU pool, shared research cluster.
- Verification: Proof of Quality (PoQ) — sampled evaluator nodes score
  candidate outputs; consensus determines reward/penalty.
- Incentive: Credit-based (no crypto), redeemable against compute contributed.
- Fault tolerance: Redundant expert assignment (send the same expert
  request to 2 peers; accept first valid response).

### Tier 3: Open Crowd (untrusted, volunteer network)

- Peers: Unbounded, open membership.
- Use case: Public inference commons (Folding@Home for LLMs).
- Verification: Optimistic execution + eventual verification
  (IETF ODSI-style). Incorrect executions are detected within the same
  token step or shortly thereafter, enabling rapid fallback before
  incorrect results propagate.
- Sybil resistance: Economic stake or proof-of-work identity, not
  administrative approval.
- Incentive: Token-based rewards with dynamic pricing (PolyLink model).
- Fault tolerance: Continuous churn is assumed, not avoided. Scalability
  under churn is achieved by decentralized participant discovery, adaptive
  assignment based on observed availability, and conservative state migration.

## The MoE Advantage

EdgeForge's MoE architecture is uniquely suited to crowd computing:

- A volunteer with a single RTX 3090 (24 GB VRAM) can host 2–3 Q4_K_M
  Mixtral experts comfortably (~1.5 GB each).
- Only ~25% of parameters activate per token, so the network does not pay
  for dormant compute.
- The router can dispatch to the cheapest available expert provider rather
  than the nearest peer.
- Experts are independently activated sub-networks — unlike dense tensor
  parallelism, no inter-expert coordination is needed mid-token.

## Architectural Impact

### New Crate: `edgeforge-crowd`

Implements:
1. Peer Reputation Graph — feeds into the streaming graph (ADR-014).
2. Verification Pipeline — pluggable verifiers (PoQ, TIQE, activation
   checksums) selectable per tier.
3. Incentive Ledger — credit accounting for Tier 2, token accounting
   for Tier 3. No on-chain dependency by default (air-gapped principle);
   optional blockchain settlement via a feature flag.
4. Churn-Resilient Scheduler — extends the L5 scheduler with
   expert-level replication (k-of-n redundant execution) and
   timeout-driven reassignment.

### Extended `InferenceEngine` Trait

```rust
pub enum ExecutionMode {
    Local,
    DistributedTrusted,             // Tier 1
    DistributedCooperative {        // Tier 2
        verification: VerificationPolicy,
        redundancy: RedundancyPolicy,
    },
    DistributedCrowd {              // Tier 3
        verification: VerificationPolicy,
        incentive: IncentivePolicy,
        churn: ChurnPolicy,
    },
}
```

The `LoadedModel` carries a distribution manifest specifying which experts
are available on which tiers, with fallback order.

## Honesty Contract (per ADR-008 / R-06)

When the user enables any tier above Local, the UI surfaces this warning
verbatim:

> Distributed mode increases capacity, not speed.
> Verification adds latency. Only enable distributed mode for models that
> cannot fit locally. Tier 3 (Open Crowd) is experimental and may produce
> incorrect outputs that are corrected retroactively.

The user must click "I understand" before the tier is activated.

## Architectural Invariants

1. Tier 3 is opt-in only. No code path may auto-enable it.
2. Tier 3 peers never see raw prompts — only decontextualized expert
   activations (R-20).
3. Verification never blocks the token stream beyond 50 ms (ADR-018,
   R-21). The `X-EdgeForge-Verification: pending` header is the escape valve.
4. Tier 3 is labeled "Experimental" in the UI. Persistent watermark.

## Consequences

Positive:
- EdgeForge scales from a single laptop to a global volunteer network
  without changing the core inference engine.
- The MoE architecture's natural expert-level parallelism makes crowd
  computing viable where dense tensor parallelism would not be.
- Tier 1 users see no behavior change from v1.0.

Negative:
- Three tiers means three verification code paths to maintain.
- Tier 3's optimistic verification can produce incorrect outputs that are
  corrected retroactively — users must understand this (the honesty
  contract is the mitigation).
- The incentive ledger introduces accounting complexity; Tier 3's
  token-based system may require blockchain settlement (feature-flagged,
  off by default per ADR-008).

## Risk Mitigation

| Risk | Mitigation |
|---|---|
| R-20: Prompt leakage / model extraction | Tier 3 sees only decontextualized activations; activation-level encryption; prompt content never in `ControlEvent` |
| R-21: Verification latency exceeds SSE budget | 50 ms deadline + async pending/corrected headers (ADR-018) |
| R-06: "2 nodes = 2× speed" misconception | Honesty contract verbatim in UI; user must click "I understand" |
