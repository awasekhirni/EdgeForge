# EdgeForge — Complete Setup Instructions

**Version:** 14.0
**Date:** 2026-10-31

This file provides detailed, step-by-step setup instructions for EdgeForge.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Quick Start (5 Minutes)](#2-quick-start-5-minutes)
3. [Build from Source](#3-build-from-source)
4. [Docker Deployment](#4-docker-deployment)
5. [Nix Hermetic Build](#5-nix-hermetic-build)
6. [Configuration](#6-configuration)
7. [Importing a Model](#7-importing-a-model)
8. [Running Inference](#8-running-inference)
9. [API Reference](#9-api-reference)
10. [Python SDK](#10-python-sdk)
11. [Desktop App](#11-desktop-app)
12. [Testing](#12-testing)
13. [Enabling Advanced Features](#13-enabling-advanced-features)
14. [Troubleshooting](#14-troubleshooting)
15. [Feature Flags Reference](#15-feature-flags-reference)

---

## 1. Prerequisites

### Hardware

| Component | Minimum | Recommended |
|---|---|---|
| CPU | 4 cores (x86_64 or ARM64) | 8+ cores |
| RAM | 16 GB | 32 GB |
| SSD | **2 TB NVMe Gen4** (ADR-002) | 4 TB NVMe Gen4/Gen5 |
| GPU | Optional (CUDA/Metal/ROCm) | Discrete GPU ≥ 16 GB VRAM |
| OS | macOS 13+ / Windows 11 / Ubuntu 22.04+ | Latest stable |

### Software

| Tool | Version | Purpose |
|---|---|---|
| Rust | 1.82+ | Build toolchain |
| CMake | 3.18+ | Building llama.cpp (optional) |
| C++ compiler | g++ 11+ or clang 14+ | Building llama.cpp (optional) |
| Node.js | 20 LTS | Desktop UI (optional) |
| Python | 3.9+ | Python SDK (optional) |
| Nix | 2.10+ | Hermetic builds (optional) |

---

## 2. Quick Start (5 Minutes)

### Step 1: Install Rust

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"

# Windows: Download rustup-init.exe from https://rustup.rs
```

Verify:
```bash
rustc --version   # Must be 1.82+
cargo --version
```

### Step 2: Clone and Build

```bash
git clone https://github.com/your-org/edgeforge.git
cd edgeforge
cargo build --release --bin edgeforge
```

### Step 3: Start the API Server

```bash
./target/release/edgeforge serve
```

You should see:
```
[edgeforge] INFO  edgeforge_api::server: listening on 127.0.0.1:11434
```

### Step 4: Test the API

```bash
# Health check
curl http://127.0.0.1:11434/health
# {"status":"ok","version":"0.1.0"}

# List models (empty initially)
curl http://127.0.0.1:11434/v1/models
# {"object":"list","data":[]}
```

### Step 5: Use with OpenAI SDK

```python
pip install openai

python3 -c "
from openai import OpenAI
client = OpenAI(base_url='http://127.0.0.1:11434/v1', api_key='not-needed')
r = client.chat.completions.create(
    model='YOUR_MODEL_ID',
    messages=[{'role':'user','content':'Hello!'}],
    stream=True
)
for c in r:
    print(c.choices[0].delta.content or '', end='', flush=True)
print()
"
```

---

## 3. Build from Source

### Step 1: Install Build Tools

**macOS:**
```bash
xcode-select --install
brew install cmake git pkg-config
```

**Ubuntu / Debian:**
```bash
sudo apt update
sudo apt install -y build-essential cmake git pkg-config libssl-dev \
    libsqlite3-dev libclang-dev llvm
```

**Windows** (PowerShell as Admin):
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools"
winget install Git.Git
winget install Kitware.CMake
```

### Step 2: Clone

```bash
git clone --recurse-submodules https://github.com/your-org/edgeforge.git
cd edgeforge

# If you forgot --recurse-submodules:
git submodule update --init --recursive
```

### Step 3: Build (Mock Engine — No llama.cpp needed)

```bash
cargo build --release --bin edgeforge
# Takes 10-30 minutes for first build (LTO=fat)
```

### Step 4: Build with Real llama.cpp (Optional)

```bash
# 1. Build llama.cpp
./scripts/build-llama.sh
# Takes 5-15 minutes

# 2. Build EdgeForge with llama-backend feature
cargo build --release --bin edgeforge --features edgeforge-runtime/llama-backend
```

### Step 5: Verify

```bash
./target/release/edgeforge --help
./target/release/edgeforge config --json
```

---

## 4. Docker Deployment

### Quick Start

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

### Manual Docker Build

```bash
docker build -t edgeforge:latest .
docker run -d \
  --name edgeforge \
  -p 11434:11434 \
  -v edgeforge-data:/data \
  -v /path/to/models:/models \
  edgeforge:latest
```

### Docker Environment Variables

```yaml
environment:
  - EDGEFORGE_DATA=/data
  - RUST_LOG=info
  - EDGEFORGE_API__LISTEN_ADDR=0.0.0.0:11434    # Accept external connections
  - EDGEFORGE_API__REQUIRE_API_KEY=true           # Enable API key auth
  - EDGEFORGE_STORAGE__DEFAULT_PRECISION=fp16     # ADR-003 default
```

---

## 5. Nix Hermetic Build

### Enter Dev Shell

```bash
nix develop
# Provides Rust 1.82 + all system deps pinned by hash
```

### Build

```bash
nix build
# Binary at ./result/bin/edgeforge
./result/bin/edgeforge serve
```

### Reproducibility

The `flake.nix` pins:
- Rust 1.82.0 via `rust-overlay`
- All system libraries by hash via `nixpkgs`
- Build inputs via `naersk` with `singleStep = true`

Release artifacts are bit-for-bit reproducible from the same `git SHA + flake.lock`.

---

## 6. Configuration

### Config File Location

| Platform | Path |
|---|---|
| Linux | `~/.config/edgeforge/config.toml` |
| macOS | `~/Library/Application Support/edgeforge/config.toml` |
| Windows | `%APPDATA%\edgeforge\config.toml` |

Or set `EDGEFORGE_CONFIG` environment variable.

### Default Config

```toml
[network]
mode = "air_gapped"          # ADR-008: air-gapped default
telemetry = false

[storage]
min_ssd_gib = 2048           # ADR-002: 2 TB minimum
default_precision = "fp16"   # ADR-003: FP16 default
prefetch_depth_layers = 1
kv_cache_budget_gib = 8
use_direct_io = true         # Bypass OS page cache
thermal_poll_interval_secs = 5

[api]
listen_addr = "127.0.0.1:11434"
require_api_key = false
rate_limit_per_minute = 600

[observability]
otlp_enabled = false         # ADR-012: off by default
log_level = "info"

[updates]
channel = "stable"           # ADR-013: stable + beta only
auto_install = false
```

### View Active Config

```bash
edgeforge config --json
# or
edgeforge config  # TOML format
```

### Environment Variable Overrides

All config values can be overridden via environment variables prefixed with `EDGEFORGE_`:

```bash
EDGEFORGE_API__LISTEN_ADDR=0.0.0.0:11434 edgeforge serve
EDGEFORGE_STORAGE__DEFAULT_PRECISION=q4 edgeforge serve
```

---

## 7. Importing a Model

### Via CLI

```bash
edgeforge model import --path /path/to/model.gguf
```

This command:
1. Verifies the file exists
2. Computes SHA-256 hash (streaming, 1 MiB chunks)
3. Auto-detects precision from filename (Q4_K_M → Q4, FP16 → FP16)
4. Auto-detects architecture from filename (mixtral → MoE, rwkv → RWKV)
5. Inserts into the SQLite registry
6. Returns a Model ID

### Via API (Phase 12+)

```bash
curl -X POST http://127.0.0.1:11434/v1/files \
  -F "file=@/path/to/model.gguf"
```

### List Models

```bash
edgeforge model list
```

Output:
```
------------------------------------------------------------------------------------
ID                         NAME                 PRECISION   SIZE (GiB)
------------------------------------------------------------------------------------
01J9XXXXXXXXXXXXXXXXXXXXX  model-name           fp16           25.50
------------------------------------------------------------------------------------
Total: 1 model(s)
```

### Verify a Model

```bash
edgeforge model verify --id 01J9XXXXXXXXXXXXXXXXXXXXX
```

### Auto-Detection Table

| Filename Contains | Detected Precision | Detected Architecture |
|---|---|---|
| Q4, q4 | Q4 | — |
| Q8, q8 | Q8 | — |
| FP16, F16 | FP16 | — |
| BF16 | BF16 | — |
| FP32, F32 | FP32 | — |
| moe, mixtral, deepseek | — | MoE |
| rwkv | — | RWKV |
| mamba | — | Mamba |
| (other) | FP16 (default) | Dense (default) |

---

## 8. Running Inference

### Via CLI (One-Shot)

```bash
edgeforge run --model 01J9XXXXXXXXXXXXXXXXXXXXX --prompt "Hello, world!"
```

### Via API (Non-Streaming)

```bash
curl -X POST http://127.0.0.1:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "01J9XXXXXXXXXXXXXXXXXXXXX",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": false,
    "max_tokens": 512
  }'
```

### Via API (SSE Streaming)

```bash
curl -N -X POST http://127.0.0.1:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "01J9XXXXXXXXXXXXXXXXXXXXX",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

Response:
```
data: {"id":"chatcmpl-...","choices":[{"delta":{"content":"Hello"}}]}

data: {"id":"chatcmpl-...","choices":[{"delta":{"content":" world"}}]}

data: [DONE]
```

### Via OpenAI Python SDK

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:11434/v1",
    api_key="not-needed",
)

response = client.chat.completions.create(
    model="01J9XXXXXXXXXXXXXXXXXXXXX",
    messages=[{"role": "user", "content": "Hello!"}],
    stream=True,
    max_tokens=512,
    temperature=0.7,
)
for chunk in response:
    print(chunk.choices[0].delta.content or "", end="", flush=True)
print()
```

---

## 9. API Reference

### Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/health` | Liveness probe |
| GET | `/v1/models` | List imported models |
| GET | `/v1/models/:id` | Get a specific model |
| POST | `/v1/chat/completions` | Chat completion (SSE + JSON) |
| POST | `/v1/completions` | Text completion |
| POST | `/v1/embeddings` | RAG embeddings |
| POST | `/v1/files` | RAG file upload (multipart) |
| GET | `/v1/webhooks` | List webhooks |
| POST | `/v1/webhooks` | Register a webhook |

### SSE Headers

- `X-EdgeForge-Verification: verified` — Token verified
- `X-EdgeForge-Verification: pending` — Verification pending (Tier 2/3)
- `X-EdgeForge-Verification: corrected` — This token corrects a previous error

### Authentication

By default, no API key is required (localhost-only). To enable:

```toml
[api]
require_api_key = true
```

Then send `Authorization: Bearer <key>` header with each request.

### Rate Limiting

Default: 600 requests/minute per IP. Configure:

```toml
[api]
rate_limit_per_minute = 1200
```

---

## 10. Python SDK

### Install

```bash
pip install edgeforge
```

### Usage

```python
import edgeforge

client = edgeforge.Client("http://127.0.0.1:11434/v1")

# Chat
response = client.chat(model="my-model", prompt="Hello!")
print(response.text)

# List models
models = client.list_models()
for m in models:
    print(f"{m.id}: {m.name} ({m.precision})")

# Check health
health = client.health()
print(f"Status: {health.status}, Version: {health.version}")

# Import a model
result = client.import_model("/path/to/model.gguf")
print(f"Imported: {result.model_id}")
```

---

## 11. Desktop App

### Prerequisites

- Node.js 20 LTS
- Tauri CLI (`npm install -g @tauri-apps/cli`)

### Run in Dev Mode

```bash
cd crates/edgeforge-app/ui
npm install
npm run tauri dev
```

### Build for Production

```bash
cd crates/edgeforge-app/ui
npm run tauri build
```

### First-Run Wizard

The desktop app shows a first-run hardware wizard that:
1. Detects CPU, RAM, VRAM, SSD, and GPU
2. Shows predicted tok/s for sample models
3. Recommends the best-fit model
4. Confirms SSD meets 2 TB NVMe minimum (ADR-002)

---

## 12. Testing

### Run All Tests

```bash
cargo test --workspace
```

### Run Tests for a Specific Crate

```bash
cargo test -p edgeforge-runtime
cargo test -p edgeforge-storage
cargo test -p edgeforge-promptsec
```

### Test Count

| Crate | Tests | What's Tested |
|---|---|---|
| edgeforge-storage | 31 | PlatformIo, expert cache, weight pager, KV cache + SSD spill, thermal, VRAM, wear |
| edgeforge-runtime | 35 | MockInferenceEngine, speculation, router hook, batching, priority, profiler, e2e |
| edgeforge-promptsec | 23 | Spotlighting, guardrails, quarantine reader, dual-LLM pipeline |
| edgeforge-orchestration | 12 | Models CRUD, jobs, lifecycle, presets, scheduler, event bus |
| edgeforge-api | 19 | Auth, rate limit, verification, logging, completions, embeddings, files, webhooks |
| edgeforge-security | 9 | SHA-256, manifest signatures, license, sandbox, crypto |
| edgeforge-crowd | 9 | Reputation, ledger, coordinator, churn scheduler |
| edgeforge-kpi | 8 | QPPOs, traceability, carbon router |
| edgeforge-graph | 8 | GraphCore, ingest, query |
| edgeforge-app | 12 | DesktopApp, IPC commands, config |
| edgeforge-sdk-python | 7 | Client chat/list/health/import |
| edgeforge-net | 3 | Transport stub, mesh, peer address |
| edgeforge-runtime-sys | 6 | Stub returns, FfiError, params |
| edgeforge-core | 3 | Config defaults, validation, presets |
| edgeforge-core-legacy | 2 | Precision monotonicity, error codes |
| edgeforge-autonomic | 2 | SPC classification |
| edgeforge-observability | 9 | FunctionLogger, guard, dump, recent |
| **Total** | **280** | **All passing** |

### Run Kani Formal Verification (ADR-025)

```bash
# Install Kani
curl -L -o kani.tar.gz https://github.com/model-checking/kani/releases/latest/download/kani-1.10.0-x86_64-unknown-linux-gnu.tar.gz
tar xzf kani.tar.gz
export PATH="$PWD/kani-1.10.0/bin:$PATH"

# Run proof harnesses
cargo kani --workspace
```

---

## 13. Enabling Advanced Features

### Real llama.cpp Inference

```bash
# 1. Pull llama.cpp submodule
git submodule update --init --recursive

# 2. Build llama.cpp
./scripts/build-llama.sh

# 3. Build with feature
cargo build --release --bin edgeforge --features edgeforge-runtime/llama-backend

# 4. Run
./target/release/edgeforge serve
```

### OpenTelemetry Export (ADR-012)

```bash
cargo build --release --features edgeforge-observability/telemetry-otlp
```

Config:
```toml
[observability]
otlp_enabled = true
otlp_endpoint = "http://localhost:4317"
```

### QUIC Mesh Transport (ADR-010)

```bash
cargo build --release --features edgeforge-net/quic-transport
```

### mDNS Discovery

```bash
cargo build --release --features edgeforge-net/mdns
```

### Combining Features

```bash
cargo build --release --bin edgeforge \
  --features edgeforge-runtime/llama-backend \
  --features edgeforge-observability/telemetry-otlp \
  --features edgeforge-net/quic-transport
```

---

## 14. Troubleshooting

### "LlamaBackend requires the llama-backend feature"

Use `MockInferenceEngine` (default) or enable the feature:
```bash
cargo build --features edgeforge-runtime/llama-backend
```

### "ffi/llama.cpp/ is not present"

```bash
git submodule update --init --recursive
```

### "cmake: command not found"

- macOS: `brew install cmake`
- Linux: `sudo apt install cmake`
- Windows: `winget install Kitware.CMake`

### "error: 'omp.h' file not found"

- macOS: `brew install libomp`
- Linux: `sudo apt install libomp-dev`

### Model import fails

```bash
# Verify file exists
ls -la /path/to/model.gguf

# Check disk space (need 1.2× model size)
df -h
```

### API returns 501 Not Implemented

Enable the appropriate feature flag. See [Section 13](#13-enabling-advanced-features).

### SSE stream returns mock output (reversed prompt)

You're using `MockInferenceEngine` (default). Enable real inference:
```bash
cargo build --features edgeforge-runtime/llama-backend
```

### SQLite "no such table" error

Run migrations:
```bash
edgeforge serve  # Automatically runs migrations on startup
```

### Rate limited (429)

Increase the rate limit:
```toml
[api]
rate_limit_per_minute = 1200
```

### SSD thermal throttling

EdgeForge automatically throttles at 70°C and pauses at 80°C. Ensure adequate SSD cooling.

---

## 15. Feature Flags Reference

| Feature | Crate | Purpose | Default |
|---|---|---|---|
| `llama-backend` | edgeforge-runtime | Real llama.cpp FFI | Off |
| `telemetry-otlp` | edgeforge-observability | OpenTelemetry OTLP export | Off |
| `quic-transport` | edgeforge-net | Real QUIC mesh + mTLS | Off |
| `mdns` | edgeforge-net | mDNS peer discovery | Off |
| `tauri-shell` | edgeforge-app | Real Tauri 2 desktop shell | Off |
| `python` | edgeforge-sdk-python | Real PyO3 bindings | Off |

### Build Commands Summary

```bash
# Minimal build (mock engine, no external deps)
cargo build --release --bin edgeforge

# Full build with real llama.cpp
cargo build --release --bin edgeforge --features edgeforge-runtime/llama-backend

# With telemetry
cargo build --release --bin edgeforge \
  --features edgeforge-runtime/llama-backend \
  --features edgeforge-observability/telemetry-otlp

# Run tests
cargo test --workspace

# Run tests with llama-backend
cargo test --workspace --features edgeforge-runtime/llama-backend

# Build Docker image
docker build -t edgeforge:latest .

# Nix hermetic build
nix build

# Dev bootstrap
./scripts/bootstrap-dev.sh
```

---

**This setup guide covers everything from quick start to production deployment. For architecture details, see `docs/functional-flow.md`. For ADRs, see `docs/adr/`.**
