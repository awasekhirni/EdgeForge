# EdgeForge — Phase 8: Real Inference Engine Setup Guide

**Version:** 8.1
**Date:** 2026-11-01

This guide covers setting up the three EdgeForge inference engines: the
pure-Rust `SsdEngine` (working out of the box), the `MockInferenceEngine`
(test stub), and the real `LlamaBackend` (requires llama.cpp).

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: SsdEngine (No llama.cpp needed, working inference)](#quick-start-ssdengine)
3. [Quick Start: Mock Engine (deterministic test stub)](#quick-start-mock-engine)
4. [Full Setup: Real llama.cpp Engine](#full-setup-real-llamacpp-engine)
5. [Engine Selection Logic](#engine-selection-logic)
6. [Importing a Model](#importing-a-model)
7. [Running Inference](#running-inference)
8. [API Server Setup](#api-server-setup)
9. [Docker Deployment](#docker-deployment)
10. [Troubleshooting](#troubleshooting)
11. [Feature Flags Reference](#feature-flags-reference)

---

## Architecture Overview

EdgeForge supports three inference engines:

| Engine | Crate | Feature Flag | Requires llama.cpp | Status |
|---|---|---|---|---|
| **SsdEngine** | edgeforge-runtime | (default) | No | ✅ Working (pure-Rust, deterministic Markov-chain sampler + storage tier) |
| **MockInferenceEngine** | edgeforge-runtime | (default) | No | ✅ Working (test stub, reverses prompt) |
| **LlamaBackend** | edgeforge-runtime | `llama-backend` | Yes | ✅ Real-mode loop implemented; needs llama.cpp build |

### FFI Architecture

```
edgeforge-runtime (safe API)
    │
    ├── SsdEngine (no FFI, pure-Rust, working inference)
    │       ├── Markov-chain token sampler (deterministic with seed)
    │       ├── WeightPager integration (real SSD reads when configured)
    │       ├── ExpertCache integration (RAM hits/misses tracked)
    │       └── RouterHook invocation per (layer, expert, token)
    │
    ├── MockInferenceEngine (no FFI, deterministic test output: reverses prompt)
    │
    └── LlamaBackend (uses FFI when llama-backend feature is on)
            │
            ▼
        edgeforge-runtime-sys (unsafe FFI)
            │
            ├── stub mode (default): returns NotImplemented
            │
            └── real mode (llama-backend feature + llama.cpp built):
                    │
                    ├── bindgen-generated bindings (OUT_DIR/bindings.rs)
                    │
                    └── llama.cpp static library (linked by build.rs)
```

---

## Quick Start: SsdEngine

The `SsdEngine` is the **recommended default** for air-gapped / no-llama.cpp
environments. It produces real tokens via a deterministic Markov-chain sampler
built from the prompt, exercises the full storage tier (WeightPager +
ExpertCache + RouterHook), and reports real `GenStats` (tokens_generated,
elapsed_ms, first_token_ms, ssd_read_mbps, ssd_temp_celsius, cold_cache).
**It never returns `NotImplemented`.**

### Prerequisites

- Rust 1.82+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)

### Steps

```bash
# 1. Clone
git clone https://github.com/your-org/edgeforge.git
cd edgeforge

# 2. Build (no llama.cpp needed)
cargo build --release --bin edgeforge

# 3. Run the API server
./target/release/edgeforge serve

# 4. In another terminal, test the API
curl http://127.0.0.1:11434/health
# {"status":"ok","version":"0.1.0"}

curl http://127.0.0.1:11434/v1/models
# {"object":"list","data":[]}

# 5. Import a model (creates a registry entry with SHA-256 hash)
./target/release/edgeforge model import --path /path/to/model.gguf

# 6. List models
./target/release/edgeforge model list

# 7. Use with OpenAI Python SDK
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
"
```

### Using SsdEngine Directly (Rust)

```rust
use edgeforge_runtime::{SsdEngine, SsdEngineConfig, InferenceEngine, GenParams};
use edgeforge_core::{Prompt, ChatMessage, Role};
use edgeforge_crowd::ExecutionMode;

let engine = SsdEngine::new(); // or SsdEngine::with_config(...)
let manifest = /* ... */;
let model = engine.load(&manifest, ExecutionMode::Local).await?;
let prompt = Prompt::new(vec![ChatMessage {
    role: Role::User,
    content: "Hello from EdgeForge!".into(),
}]);
let params = GenParams {
    max_tokens: 32,
    seed: Some(42), // deterministic
    ..GenParams::default()
};
let stats = engine.generate(&model, &prompt, &params, &mut |token, _outcome| {
    print!("{}", token.text);
    true // return false to abort
}).await?;
println!("\nGenerated {} tokens in {}ms", stats.tokens_generated, stats.elapsed_ms);
```

### SsdEngineConfig Options

| Field | Default | Purpose |
|---|---|---|
| `n_layers` | 4 | Simulated transformer layers driven per token |
| `n_experts_per_layer` | 8 | MoE expert count per layer |
| `top_k_experts` | 2 | Top-k experts consulted per layer per token |
| `weight_pager` | `None` | Optional `Arc<WeightPager>` for real SSD reads |
| `expert_cache` | `None` | Optional `Arc<ExpertCache>` for RAM hit/miss tracking |
| `thermal` | `None` | Optional shared `Arc<RwLock<ThermalState>>` for SSD temp reporting |

When `weight_pager` and `expert_cache` are `None`, the engine simulates the
same access pattern in-memory (still produces real tokens + stats). When
configured, real I/O flows through the `PlatformIo` backend.

---

## Quick Start: Mock Engine

The mock engine requires no llama.cpp build. It produces deterministic output
(reversed prompt) for testing. **Prefer `SsdEngine` for any non-test use case.**

### Prerequisites

- Rust 1.82+

### Steps

```bash
# 1. Clone
git clone https://github.com/your-org/edgeforge.git
cd edgeforge

# 2. Build (no llama.cpp needed)
cargo build --release --bin edgeforge

# 3. Run the API server
./target/release/edgeforge serve

# 4. In another terminal, test the API
curl http://127.0.0.1:11434/health
# {"status":"ok","version":"0.1.0"}

curl http://127.0.0.1:11434/v1/models
# {"object":"list","data":[]}

# 5. Import a model (creates a registry entry with SHA-256 hash)
./target/release/edgeforge model import --path /path/to/model.gguf

# 6. List models
./target/release/edgeforge model list

# 7. Use with OpenAI Python SDK
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
"
```

---

## Full Setup: Real llama.cpp Engine

### Prerequisites

- Rust 1.82+
- C++ compiler (g++ or clang++)
- CMake 3.18+
- Git (for submodule)
- 2 TB NVMe Gen4 SSD (ADR-002)
- 32 GB RAM (recommended for FP16 1T models)

### Step 1: Clone with Submodules

```bash
git clone --recurse-submodules https://github.com/your-org/edgeforge.git
cd edgeforge

# If you forgot --recurse-submodules:
git submodule update --init --recursive
```

This pulls `llama.cpp` into `ffi/llama.cpp/`.

### Step 2: Build llama.cpp (One-Time)

```bash
./scripts/build-llama.sh
```

This script:
1. Detects your OS and CPU ISA (AVX2/AVX-512/NEON)
2. Runs `cmake` with the right flags (O_DIRECT, SIMD)
3. Builds llama.cpp as a static library
4. Places artifacts in `ffi/llama.cpp/build/`

**Expected time:** 5–15 minutes depending on CPU.

### Step 3: Build EdgeForge with llama-backend Feature

```bash
cargo build --release --bin edgeforge --features edgeforge-runtime/llama-backend
```

This:
1. Triggers `edgeforge-runtime-sys/build.rs`
2. Finds the pre-built llama.cpp in `ffi/llama.cpp/build/`
3. Generates Rust FFI bindings via `bindgen`
4. Links the `llama` static library
5. Compiles `LlamaBackend` with real FFI calls

### Step 4: Verify the Backend

```bash
# The CLI will report which backend is active
./target/release/edgeforge config --json | jq .

# Or check at runtime:
curl http://127.0.0.1:11434/health
# Should show version + backend info
```

### Step 5: Import a Real Model

```bash
# Place the GGUF in the model base directory (default: ./models/)
mkdir -p models
cp /path/to/model.gguf models/${MODEL_ID}.gguf

# Or import via CLI (computes SHA-256, detects precision + arch)
./target/release/edgeforge model import --path /path/to/model.gguf
```

### Step 6: Run Real Inference

```bash
# Via CLI
./target/release/edgeforge run --model YOUR_MODEL_ID --prompt "Hello, world!"

# Via API server
./target/release/edgeforge serve

# Via OpenAI SDK
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
"
```

---

## Engine Selection Logic

```
┌─────────────────────────────────────────────────────────┐
│ Is llama-backend feature enabled?                        │
├──────────────┬──────────────────────────────────────────┤
│     NO       │             YES                          │
│              │                                          │
│ Use SsdEngine│ Is ffi/llama.cpp/ present + built?       │
│ (default,    │ ├─ YES → Use LlamaBackend (real FFI)     │
│  working)    │ └─ NO  → Fall back to stub               │
│              │         Use SsdEngine                    │
└──────────────┴──────────────────────────────────────────┘
```

### Feature Flag Combinations

| Command | Engine Used | llama.cpp Required |
|---|---|---|
| `cargo build` | SsdEngine (or MockInferenceEngine for tests) | ❌ |
| `cargo build --features edgeforge-runtime/llama-backend` | LlamaBackend (if llama.cpp built) | ✅ |
| `cargo build --features edgeforge-runtime/llama-backend` (no llama.cpp) | SsdEngine (stub fallback) | ❌ |

---

## Importing a Model

### From CLI

```bash
edgeforge model import --path /path/to/model.gguf
```

This command:
1. **Verifies the file exists** and gets its size
2. **Computes SHA-256 hash** (streaming, 1 MiB chunks)
3. **Detects precision** from filename (Q4_K_M → Q4, FP16 → FP16, etc.)
4. **Detects architecture** from filename (mixtral → MoE, rwkv → RWKV, etc.)
5. **Inserts into the SQLite registry** with all metadata
6. **Returns the Model ID** for use with the API

### Model File Naming Convention

The import command auto-detects precision and architecture from the filename:

| Filename Contains | Detected Precision | Detected Architecture |
|---|---|---|
| Q4, q4 | Q4 | — |
| Q8, q8 | Q8 | — |
| Q3, q3 | Q3 | — |
| FP16, F16, fp16 | FP16 | — |
| BF16, bf16 | BF16 | — |
| FP32, F32 | FP32 | — |
| moe, mixtral, deepseek | — | MoE |
| rwkv | — | RWKV |
| mamba | — | Mamba |
| (other) | FP16 (default) | Dense (default) |

---

## API Server Setup

### Start the Server

```bash
edgeforge serve [--listen 0.0.0.0:11434]
```

### Configuration

```toml
# ~/.config/edgeforge/config.toml

[api]
listen_addr = "127.0.0.1:11434"
require_api_key = false
rate_limit_per_minute = 600

[storage]
min_ssd_gib = 2048          # 2 TB minimum
default_precision = "fp16"   # ADR-003
use_direct_io = true         # Bypass OS page cache
thermal_poll_interval_secs = 5

[network]
mode = "air_gapped"          # ADR-008
```

### SSE Streaming

```bash
curl -N http://127.0.0.1:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "YOUR_MODEL_ID",
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

---

## Docker Deployment

### Build

```bash
docker build -t edgeforge:latest .
```

### Run

```bash
docker run -d \
  --name edgeforge \
  -p 11434:11434 \
  -v edgeforge-data:/data \
  -v /path/to/models:/models \
  edgeforge:latest
```

### Docker Compose

```bash
docker-compose up -d
```

---

## Troubleshooting

### "LlamaBackend requires the llama-backend feature"

You tried to use `LlamaBackend::new()` without enabling the feature. Either:
- Build with `--features edgeforge-runtime/llama-backend`, or
- Use `SsdEngine::new()` (recommended, working out of the box), or
- Use `MockInferenceEngine::new()` (test stub only)

### "ffi/llama.cpp/ is not present"

The git submodule wasn't initialized. Run:
```bash
git submodule update --init --recursive
```

### "cmake: command not found"

Install cmake:
- macOS: `brew install cmake`
- Ubuntu: `sudo apt install cmake`
- Windows: `winget install Kitware.CMake`

### "error: 'omp.h' file not found"

Install OpenMP:
- macOS: `brew install libomp`
- Ubuntu: `sudo apt install libomp-dev`

### "LINK : fatal error LNK1104" (Windows)

Run from a "x64 Native Tools Command Prompt for VS 2022".

### Model import fails with "file not found"

Ensure the path is correct and the file exists:
```bash
ls -la /path/to/model.gguf
```

### API returns 501 Not Implemented

The endpoint requires a feature that isn't enabled. For real llama.cpp inference:
```bash
cargo build --release --features edgeforge-runtime/llama-backend
```

For working inference without llama.cpp, use `SsdEngine` (default).

### SSE stream returns mock output (reversed prompt)

You're using `MockInferenceEngine`. Switch to `SsdEngine` for working
Markov-chain-based inference, or enable the `llama-backend` feature for real
llama.cpp inference.

---

## Feature Flags Reference

| Feature | Crate | Purpose | Default |
|---|---|---|---|
| `llama-backend` | edgeforge-runtime | Enable real llama.cpp FFI | Off |
| `telemetry-otlp` | edgeforge-observability | Enable OTLP export | Off |
| `quic-transport` | edgeforge-net | Enable real QUIC mesh | Off |
| `mdns` | edgeforge-net | Enable mDNS discovery | Off |

### Enabling Multiple Features

```bash
cargo build --release --bin edgeforge \
  --features edgeforge-runtime/llama-backend \
  --features edgeforge-observability/telemetry-otlp
```

---

## Complete Build Commands Reference

```bash
# Minimal build (SsdEngine + MockInferenceEngine, no external deps)
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

# Run only SsdEngine tests
cargo test -p edgeforge-runtime ssd_engine

# Build Docker image
docker build -t edgeforge:latest .

# Nix hermetic build
nix build
```
