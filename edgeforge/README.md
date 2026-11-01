# EdgeForge
### Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
### Author: Awase Khirni Syed
> **Local-first, SSD-backed desktop runtime for frontier-scale AI models (1T parameters).**
>
> Runs 744B–1T-parameter MoE models locally using an NVMe SSD as an extended memory tier, with optional pooling of multiple machines into a distributed compute network.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust: 1.82+](https://img.shields.io/badge/rust-1.82+-orange.svg)](https://www.rust-lang.org/)
[![Tests: 55](https://img.shields.io/badge/tests-55%20passing-brightgreen.svg)](#testing)

---

## Table of Contents

1. [What is EdgeForge?](#what-is-edgeforge)
2. [Architecture](#architecture)
3. [Quick Start](#quick-start)
4. [Configuration](#configuration)
5. [API Reference](#api-reference)
6. [Testing](#testing)
7. [Deployment](#deployment)
8. [Contributing](#contributing)
9. [License](#license)

---

## What is EdgeForge?

EdgeForge is a desktop application for macOS, Windows, and Linux that runs frontier-scale AI models (744B–1T parameters) locally using an NVMe SSD as an extended memory tier.

### Key Features

- **SSD-Backed Inference** — Runs models 10× larger than VRAM by streaming expert weights from SSD (F-TIER-01 through F-TIER-06)
- **FP16 Default** — Preserves full model quality for research use (ADR-003)
- **OpenAI-Compatible API** — Drop-in replacement for cloud APIs at `127.0.0.1:11434` (ADR-007)
- **Air-Gapped Default** — No outbound network calls unless explicitly enabled (ADR-008)
- **Distributed Pool** — Tier 1 (Trusted Mesh) / Tier 2 (Cooperative) / Tier 3 (Open Crowd) (ADR-017)
- **Prompt Security** — Dual-LLM privilege separation + spotlighting + intent guardrails + quarantine reader (ADRs 020–023)
- **Streaming Graph** — Derived control plane with <10µs query latency (ADR-014)
- **Multi-Horizon Speculation** — H1/H2/H3 prediction with accuracy SLA (ADR-015)
- **Post-Quantum Ready** — Hybrid PQC (X25519 + ML-KEM-768) in beta channel (ADR-027)
- **CMMI Level 5** — 8 frozen QPPOs, Kani formal verification, MAPE-K autonomic loop (ADRs 024–032)
- **50-Year Durability** — Zero-dependency `no_std` core, Nix hermetic builds, dual implementations (ADRs 024, 028, 032)

---

## Architecture

EdgeForge uses an 11-layer architecture with 18 Rust crates:

```
L6   UI Layer                  Tauri 2 shell + React/Webview
L5.5 Autonomic Loop            MAPE-K · SPC · Antifragile test generation
L5   Orchestration             Model lifecycle · Scheduler · OpenAI-compatible API
L4   Inference Runtime         MockInferenceEngine · SpeculationEngine · GraphAwareRouterHook
L3.5 Streaming Graph           Derived control plane · <10µs queries · fallback-safe
L3   Storage Tier              WeightPager · KvCachePager (with SSD spill) · IoUring/IOCP/dispatch_io
L2.5 Crowd Coordination        Reputation · Verification · Incentives · Churn-resilient
L2   Networking                mDNS · QUIC mesh · Hybrid PQC mTLS
L1.5 Prompt Security           Dual-LLM · Spotlighting · Guardrails · Quarantine Reader
L1   Security                  Manifest verifier · Crypto-agility · Sandbox
```

### 18 Crates

| Crate | Layer | Purpose |
|---|---|---|
| `edgeforge-core` | Cross-cutting | Shared types, ControlEvent, errors, config |
| `edgeforge-core-legacy` | L0 | Zero-dep `no_std` core (50-year survivability) |
| `edgeforge-observability` | Cross-cutting | tracing + opt-in OTLP (ADR-012) |
| `edgeforge-security` | L1 | Manifest verifier, crypto-agility, sandbox |
| `edgeforge-promptsec` | L1.5 | Dual-LLM, spotlighting, guardrails, quarantine (ADRs 020–023) |
| `edgeforge-net` | L2 | QUIC mesh, mDNS, mTLS, bandwidth prober |
| `edgeforge-crowd` | L2.5 | Reputation, verification, incentives, churn (ADR-017) |
| `edgeforge-storage` | L3 | WeightPager, KvCachePager, PlatformIo, thermal |
| `edgeforge-graph` | L3.5 | Streaming property graph (ADR-014) |
| `edgeforge-runtime` | L4 | MockInferenceEngine, speculation, router hook |
| `edgeforge-runtime-sys` | L4 | Unsafe FFI to llama.cpp (ADR-001) |
| `edgeforge-orchestration` | L5 | Lifecycle, scheduler, jobs, event bus, ORM |
| `edgeforge-autonomic` | L5.5 | MAPE-K loop, SPC, test generator (ADR-029) |
| `edgeforge-kpi` | Cross-cutting | QPPOs, traceability, carbon-aware router (ADR-030) |
| `edgeforge-api` | L5 | OpenAI-compatible REST + SSE (ADR-007) |
| `edgeforge-cli` | L6 | `edgeforge` binary |
| `edgeforge-sdk-python` | L6 | PyO3 Python SDK (ADR-011) |
| `edgeforge-app` | L6 | Tauri 2 desktop shell (ADR-006) |

### 32 ADRs

See `docs/adr/` for all 32 Architecture Decision Records.

---

## Quick Start

### Prerequisites

| Component | Minimum | Recommended |
|---|---|---|
| CPU | 4 cores, x86_64 or Apple Silicon | 8+ cores |
| RAM | 16 GB | 32 GB |
| SSD | **2 TB NVMe Gen4** | 4 TB NVMe Gen4/Gen5 |
| GPU | Optional (CUDA / Metal / ROCm) | Discrete, ≥16 GB VRAM |
| OS | macOS 13+ / Windows 11 / Ubuntu 22.04+ | Latest stable |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
rustc --version  # must be 1.82+
```

### Build

```bash
git clone --recurse-submodules https://github.com/your-org/edgeforge.git
cd edgeforge
cargo build --release --bin edgeforge
```

### Run

```bash
# Start the API server
./target/release/edgeforge serve

# Test it
curl http://127.0.0.1:11434/health
curl http://127.0.0.1:11434/v1/models
```

### Use with OpenAI SDK

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:11434/v1",
    api_key="not-needed",
)

response = client.chat.completions.create(
    model="my-model",
    messages=[{"role": "user", "content": "Hello!"}],
    stream=True,
)
for chunk in response:
    print(chunk.choices[0].delta.content or "", end="", flush=True)
```

---

## Configuration

Default config location: `~/.config/edgeforge/config.toml` (or `EDGEFORGE_CONFIG` env var).

```toml
[network]
mode = "air_gapped"          # air_gapped | lan_open | global_relay
telemetry = false

[storage]
min_ssd_gib = 2048           # 2 TB minimum (ADR-002)
default_precision = "fp16"   # ADR-003
prefetch_depth_layers = 1
kv_cache_budget_gib = 8
use_direct_io = true
thermal_poll_interval_secs = 5

[api]
listen_addr = "127.0.0.1:11434"
require_api_key = false
rate_limit_per_minute = 600

[updates]
channel = "stable"           # stable | beta (ADR-013)
auto_install = false
```

### View Active Config

```bash
edgeforge config --json
```

---

## API Reference

The full OpenAPI 3.1 specification is at `resources/schemas/openapi.yaml`.

### Endpoints

| Method | Path | Description |
|---|---|---|
| POST | `/v1/chat/completions` | Chat completion (SSE streaming supported) |
| POST | `/v1/completions` | Text completion |
| POST | `/v1/embeddings` | RAG embeddings (F-DEV-06) |
| GET | `/v1/models` | List imported models |
| GET | `/v1/models/:id` | Get a specific model |
| GET | `/health` | Liveness probe |

### SSE Streaming

When `stream: true`, the response is `text/event-stream`:

```
data: {"id":"chatcmpl-...","choices":[{"delta":{"content":"Hello"}}]}

data: {"id":"chatcmpl-...","choices":[{"delta":{"content":" world"}}]}

data: [DONE]
```

The `X-EdgeForge-Verification` header indicates token verification status (ADR-018):
- `verified` — verification passed
- `pending` — verification pending (Tier 2/3)
- `corrected` — this chunk corrects a previous pending token

---

## Testing

```bash
# Run all 55 tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p edgeforge-runtime
cargo test -p edgeforge-storage
```

### Test Coverage

| Crate | Tests | What's Tested |
|---|---|---|
| edgeforge-core | 3 | Config defaults, validation, precision tiers |
| edgeforge-core-legacy | 2 | Precision monotonicity, stable error codes |
| edgeforge-security | 2 | SHA-256 integrity, sandbox policy |
| edgeforge-storage | 19 | PlatformIo, expert cache LRU, weight pager, KV cache + SSD spill, thermal |
| edgeforge-graph | 2 | GraphCore, query |
| edgeforge-runtime | 19 | MockInferenceEngine, speculation, router hook, e2e integration |
| edgeforge-orchestration | 3 | Config, lifecycle, presets |
| edgeforge-autonomic | 2 | MAPE-K SPC classification |
| **Total** | **55** | **All passing** |

---

## Deployment

### Docker

```bash
# Build and run
docker-compose up -d

# Check health
curl http://127.0.0.1:11434/health

# View logs
docker-compose logs -f edgeforge

# Stop
docker-compose down
```

The `edgeforge-data` volume persists the SQLite database and model files across restarts.

### Nix (Hermetic Builds — ADR-028)

```bash
# Enter a dev shell with all tooling pinned
nix develop

# Build the workspace
nix build
```

### Kani Formal Verification (ADR-025)

```bash
# Install Kani
curl -L -o kani.tar.gz https://github.com/model-checking/kani/releases/latest/download/kani-1.10.0-x86_64-unknown-linux-gnu.tar.gz
tar xzf kani.tar.gz
export PATH="$PWD/kani-1.10.0/bin:$PATH"

# Run proof harnesses
cargo kani --workspace
```

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup, coding standards, and PR process.

### Coding Standards

- `cargo fmt --all -- --check` — formatting must pass
- `cargo clippy --workspace -- -D warnings` — no clippy warnings
- `cargo test --workspace` — all tests must pass
- `cargo deny check` — license + cycle + ban checks
- Every public API item must carry a `[trace(...)]` attribute (ADR-030)

---

## License

Apache-2.0. See [LICENSE](./LICENSE).
