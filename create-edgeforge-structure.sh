#!/usr/bin/env bash
# Copyright 2025 Beta ORI Inc. Canada. All Rights Reserved. 
# Syed, Awase Khirni  Edgeforge folder structure bash script 
# create-edgeforge-structure.sh
# Recreates the EdgeForge workspace folder structure from the approved layout.
# Idempotent: safe to re-run. Usage: ./create-edgeforge-structure.sh [ROOT]
set -euo pipefail

ROOT="${1:-edgeforge}"

# ─── helpers ──────────────────────────────────────────────────────────────
d()  { mkdir -p "$ROOT/$1"; }

f()  {                                    # empty placeholder file
    local p="$ROOT/$1"
    mkdir -p "$(dirname "$p")"
    [[ -e "$p" ]] || touch "$p"
}

fm() {                                    # markdown stub
    local p="$ROOT/$1"
    mkdir -p "$(dirname "$p")"
    [[ -e "$p" ]] || printf '# %s\n\n<!-- TODO: fill in -->\n' \
        "$(basename "$p" .md)" > "$p"
}

fs() {                                    # shell script stub (executable)
    local p="$ROOT/$1"
    mkdir -p "$(dirname "$p")"
    if [[ ! -e "$p" ]]; then
        printf '#!/usr/bin/env bash\nset -euo pipefail\n\n# TODO\n' > "$p"
        chmod +x "$p"
    fi
}

echo "→ Creating EdgeForge structure at: $ROOT"
d .

# ═══════════════════════════════════════════════════════════════════════════
# ROOT FILES
# ═══════════════════════════════════════════════════════════════════════════
fm README.md

if [[ ! -e "$ROOT/Cargo.toml" ]]; then
cat > "$ROOT/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members  = ["crates/*"]

[workspace.package]
edition    = "2021"
rust-version = "1.82"
license    = "Apache-2.0"
EOF
fi

if [[ ! -e "$ROOT/rust-toolchain.toml" ]]; then
cat > "$ROOT/rust-toolchain.toml" <<'EOF'
[toolchain]
channel    = "stable"
components = ["rustfmt", "clippy"]
EOF
fi

if [[ ! -e "$ROOT/.gitmodules" ]]; then
cat > "$ROOT/.gitmodules" <<'EOF'
[submodule "ffi/llama.cpp"]
    path = ffi/llama.cpp
    url  = https://github.com/ggerganov/llama.cpp.git
EOF
fi

f Cargo.lock
f deny.toml
f rustfmt.toml
f clippy.toml
f flake.nix

# ═══════════════════════════════════════════════════════════════════════════
# CRATES (18)
# ═══════════════════════════════════════════════════════════════════════════

# ── edgeforge-core ────────────────────────────────────────────────────────
f crates/edgeforge-core/Cargo.toml
f crates/edgeforge-core/src/lib.rs
f crates/edgeforge-core/src/error.rs
f crates/edgeforge-core/src/config.rs
f crates/edgeforge-core/src/control_event.rs
f crates/edgeforge-core/src/types.rs
f crates/edgeforge-core/src/units.rs

# ── edgeforge-core-legacy ─────────────────────────────────────────────────
f crates/edgeforge-core-legacy/Cargo.toml
f crates/edgeforge-core-legacy/src/lib.rs
f crates/edgeforge-core-legacy/src/std_impls.rs

# ── edgeforge-observability ───────────────────────────────────────────────
f crates/edgeforge-observability/Cargo.toml
f crates/edgeforge-observability/src/lib.rs
f crates/edgeforge-observability/src/subscriber.rs
f crates/edgeforge-observability/src/otlp.rs
f crates/edgeforge-observability/src/metrics.rs
f crates/edgeforge-observability/src/span.rs

# ── edgeforge-security ────────────────────────────────────────────────────
f crates/edgeforge-security/Cargo.toml
f crates/edgeforge-security/src/lib.rs
f crates/edgeforge-security/src/manifest.rs
f crates/edgeforge-security/src/hash.rs
f crates/edgeforge-security/src/sandbox.rs
f crates/edgeforge-security/src/attestation.rs
f crates/edgeforge-security/src/encryption.rs
f crates/edgeforge-security/src/crypto_agility.rs

# ── edgeforge-promptsec ───────────────────────────────────────────────────
f crates/edgeforge-promptsec/Cargo.toml
f crates/edgeforge-promptsec/src/lib.rs
f crates/edgeforge-promptsec/src/privilege.rs
f crates/edgeforge-promptsec/src/spotlighting.rs
f crates/edgeforge-promptsec/src/guardrails.rs
f crates/edgeforge-promptsec/src/quarantine.rs

# ── edgeforge-net ─────────────────────────────────────────────────────────
f crates/edgeforge-net/Cargo.toml
f crates/edgeforge-net/src/lib.rs
f crates/edgeforge-net/src/discovery.rs
f crates/edgeforge-net/src/quic_transport.rs
f crates/edgeforge-net/src/mtls.rs
f crates/edgeforge-net/src/capability.rs
f crates/edgeforge-net/src/mesh.rs
f crates/edgeforge-net/src/pipeline.rs
f crates/edgeforge-net/src/bandwidth.rs
f crates/edgeforge-net/src/recovery.rs

# ── edgeforge-crowd ───────────────────────────────────────────────────────
f crates/edgeforge-crowd/Cargo.toml
f crates/edgeforge-crowd/src/lib.rs
f crates/edgeforge-crowd/src/coordinator.rs
f crates/edgeforge-crowd/src/verification.rs
f crates/edgeforge-crowd/src/reputation.rs
f crates/edgeforge-crowd/src/ledger.rs
f crates/edgeforge-crowd/src/churn.rs

# ── edgeforge-storage ─────────────────────────────────────────────────────
f crates/edgeforge-storage/Cargo.toml
f crates/edgeforge-storage/src/lib.rs
f crates/edgeforge-storage/src/weight_pager.rs
f crates/edgeforge-storage/src/expert_cache.rs
f crates/edgeforge-storage/src/kv_pager.rs
f crates/edgeforge-storage/src/prefetcher.rs
f crates/edgeforge-storage/src/vram_tier.rs
f crates/edgeforge-storage/src/thermal.rs
f crates/edgeforge-storage/src/wear.rs
f crates/edgeforge-storage/src/platform/mod.rs
f crates/edgeforge-storage/src/platform/iouring.rs
f crates/edgeforge-storage/src/platform/iocp.rs
f crates/edgeforge-storage/src/platform/dispatch_io.rs
f crates/edgeforge-storage/src/platform/blocking.rs

# ── edgeforge-graph ───────────────────────────────────────────────────────
f crates/edgeforge-graph/Cargo.toml
f crates/edgeforge-graph/src/lib.rs
f crates/edgeforge-graph/src/core_.rs
f crates/edgeforge-graph/src/ingest.rs
f crates/edgeforge-graph/src/query.rs
f crates/edgeforge-graph/benches/query_latency.rs

# ── edgeforge-runtime ─────────────────────────────────────────────────────
f crates/edgeforge-runtime/Cargo.toml
f crates/edgeforge-runtime/src/lib.rs
f crates/edgeforge-runtime/src/engine.rs
f crates/edgeforge-runtime/src/llama_backend.rs
f crates/edgeforge-runtime/src/ssd_engine.rs
f crates/edgeforge-runtime/src/router_hook.rs
f crates/edgeforge-runtime/src/speculation.rs
f crates/edgeforge-runtime/src/batching.rs
f crates/edgeforge-runtime/src/priority.rs
f crates/edgeforge-runtime/src/determinism.rs
f crates/edgeforge-runtime/src/profiler.rs

# ── edgeforge-runtime-sys (special: build.rs, wrapper.h) ──────────────────
f crates/edgeforge-runtime-sys/Cargo.toml
f crates/edgeforge-runtime-sys/build.rs
f crates/edgeforge-runtime-sys/wrapper.h
f crates/edgeforge-runtime-sys/src/lib.rs

# ── edgeforge-orchestration ───────────────────────────────────────────────
f crates/edgeforge-orchestration/Cargo.toml
f crates/edgeforge-orchestration/src/lib.rs
f crates/edgeforge-orchestration/src/lifecycle.rs
f crates/edgeforge-orchestration/src/scheduler.rs
f crates/edgeforge-orchestration/src/presets.rs
f crates/edgeforge-orchestration/src/mesh_coord.rs
f crates/edgeforge-orchestration/src/jobs.rs
f crates/edgeforge-orchestration/src/benchmark.rs
f crates/edgeforge-orchestration/src/quality_eval.rs
f crates/edgeforge-orchestration/src/event_bus.rs
f crates/edgeforge-orchestration/src/db.rs

# ── edgeforge-autonomic ───────────────────────────────────────────────────
f crates/edgeforge-autonomic/Cargo.toml
f crates/edgeforge-autonomic/src/lib.rs
f crates/edgeforge-autonomic/src/mape.rs
f crates/edgeforge-autonomic/src/knowledge.rs
f crates/edgeforge-autonomic/src/remediation.rs
f crates/edgeforge-autonomic/src/test_generator.rs

# ── edgeforge-kpi ─────────────────────────────────────────────────────────
f crates/edgeforge-kpi/Cargo.toml
f crates/edgeforge-kpi/src/lib.rs
f crates/edgeforge-kpi/src/qppo.rs
f crates/edgeforge-kpi/src/traceability.rs
f crates/edgeforge-kpi/src/carbon.rs

# ── edgeforge-api ─────────────────────────────────────────────────────────
f crates/edgeforge-api/Cargo.toml
f crates/edgeforge-api/src/lib.rs
f crates/edgeforge-api/src/server.rs
f crates/edgeforge-api/src/state.rs
f crates/edgeforge-api/src/dto.rs
f crates/edgeforge-api/src/dto/mod.rs
f crates/edgeforge-api/src/routes/mod.rs
f crates/edgeforge-api/src/routes/chat.rs
f crates/edgeforge-api/src/routes/completions.rs
f crates/edgeforge-api/src/routes/embeddings.rs
f crates/edgeforge-api/src/routes/models.rs
f crates/edgeforge-api/src/routes/files.rs
f crates/edgeforge-api/src/routes/webhooks.rs
f crates/edgeforge-api/src/routes/health.rs
f crates/edgeforge-api/src/middleware/mod.rs
f crates/edgeforge-api/src/middleware/error.rs
f crates/edgeforge-api/src/middleware/rate_limit.rs
f crates/edgeforge-api/src/middleware/auth.rs
f crates/edgeforge-api/src/middleware/verification.rs
f crates/edgeforge-api/src/middleware/logging.rs

# ── edgeforge-sdk-python ──────────────────────────────────────────────────
f crates/edgeforge-sdk-python/Cargo.toml
f crates/edgeforge-sdk-python/pyproject.toml
f crates/edgeforge-sdk-python/src/lib.rs

# ── edgeforge-cli (special: main.rs) ──────────────────────────────────────
f crates/edgeforge-cli/Cargo.toml
f crates/edgeforge-cli/src/main.rs
f crates/edgeforge-cli/src/output.rs
f crates/edgeforge-cli/src/commands/mod.rs
f crates/edgeforge-cli/src/commands/serve.rs
f crates/edgeforge-cli/src/commands/run.rs
f crates/edgeforge-cli/src/commands/model.rs
f crates/edgeforge-cli/src/commands/bench.rs
f crates/edgeforge-cli/src/commands/pool.rs
f crates/edgeforge-cli/src/commands/config_cmd.rs

# ── edgeforge-app (Tauri 2: backend + React UI) ───────────────────────────
f crates/edgeforge-app/Cargo.toml
f crates/edgeforge-app/tauri.conf.json
f crates/edgeforge-app/src/main.rs
f crates/edgeforge-app/src/lib.rs
f crates/edgeforge-app/src/commands.rs
f crates/edgeforge-app/src/menu.rs
f crates/edgeforge-app/ui/package.json
f crates/edgeforge-app/ui/vite.config.ts
f crates/edgeforge-app/ui/tailwind.config.ts
f crates/edgeforge-app/ui/src/App.tsx
f crates/edgeforge-app/ui/src/routes/Chat.tsx
f crates/edgeforge-app/ui/src/routes/Library.tsx
f crates/edgeforge-app/ui/src/routes/Pool.tsx
f crates/edgeforge-app/ui/src/routes/Monitor.tsx
f crates/edgeforge-app/ui/src/routes/Settings.tsx
f crates/edgeforge-app/ui/src/routes/Wizard.tsx
f crates/edgeforge-app/ui/src/components/index.ts
f crates/edgeforge-app/ui/src/hooks/index.ts
f crates/edgeforge-app/ui/src/stores/index.ts
f crates/edgeforge-app/ui/src/api/index.ts

# ═══════════════════════════════════════════════════════════════════════════
# FFI
# ═══════════════════════════════════════════════════════════════════════════
fm ffi/llama.cpp/README.md

# ═══════════════════════════════════════════════════════════════════════════
# RESOURCES
# ═══════════════════════════════════════════════════════════════════════════
# Migrations (16)
for i in \
  0001_init 0002_models 0003_jobs 0004_conversations 0005_pool_nodes \
  0006_benchmarks 0007_embeddings 0008_peer_reputation 0009_verification \
  0010_incentive 0011_benchmark_extensions 0012_prompt_audit 0013_kpi_qppo \
  0014_traceability 0015_autonomic 0016_crypto_inventory; do
  f "resources/migrations/${i}.sql"
done

# Schemas
f resources/schemas/manifest.v1.json
f resources/schemas/preset.v1.json
f resources/schemas/control_event.v1.json
f resources/schemas/openapi.yaml

# Manifests
f resources/manifests/default-publishers.json

# ═══════════════════════════════════════════════════════════════════════════
# DOCS
# ═══════════════════════════════════════════════════════════════════════════
# ADRs 0001–0032
adrs=(
  0001-rust-plus-llama-cpp-ffi
  0002-2tb-nvme-minimum
  0003-fp16-default-precision
  0004-precision-tier-matrix
  0005-sqlite-persistence
  0006-tauri-2-shell
  0007-openai-compatible-api
  0008-air-gapped-default
  0009-llama-cpp-stable-pin
  0010-quic-mesh-transport
  0011-python-prebuilt-wheels
  0012-opentelemetry-opt-in
  0013-stable-beta-release-channels
  0014-streaming-graph
  0015-multi-horizon-speculation
  0016-workload-aware-speculation
  0017-tiered-crowd-computing
  0018-async-verification
  0019-control-event-schema
  0020-dual-llm-privilege-separation
  0021-spotlighting
  0022-intent-guardrails
  0023-quarantine-reader-llm
  0024-legacy-first-core-design
  0025-formal-verification-gate
  0026-carbon-aware-routing
  0027-hybrid-post-quantum-mesh
  0028-hermetic-reproducible-builds
  0029-mape-k-autonomic-loop
  0030-architecture-kpi-dashboard
  0031-compute-substrate-abstraction
  0032-multiple-implementations
)
for adr in "${adrs[@]}"; do
  fm "docs/adr/${adr}.md"
done

# Architecture
fm docs/architecture/01-overview.md
fm docs/architecture/02-storage-tier.md
fm docs/architecture/03-runtime.md
fm docs/architecture/04-mesh.md
fm docs/architecture/05-streaming-graph.md
fm docs/architecture/06-crowd-tiers.md
fm docs/architecture/07-prompt-security.md

# Setup
fm docs/setup/01-prerequisites.md
fm docs/setup/02-build-from-source.md
fm docs/setup/03-first-run.md
fm docs/setup/04-troubleshooting.md

# API docs
fm docs/api/README.md

# ═══════════════════════════════════════════════════════════════════════════
# SCRIPTS
# ═══════════════════════════════════════════════════════════════════════════
fs scripts/build-llama.sh
fs scripts/gen-bindings.sh
fs scripts/install.sh
fs scripts/bootstrap-dev.sh

# ═══════════════════════════════════════════════════════════════════════════
# TESTS
# ═══════════════════════════════════════════════════════════════════════════
f tests/e2e/chat_streaming.rs
f tests/e2e/model_import.rs
f tests/e2e/distributed_pool.rs
f tests/e2e/io_conformance.rs
f tests/e2e/graph_query_latency.rs
f tests/e2e/speculation_acceptance.rs
f tests/e2e/crowd_verification.rs
f tests/e2e/promptsec_injection.rs
f tests/e2e/promptsec_quarantine.rs

f tests/kani/platform_io.rs

fm tests/fixtures/models/README.md
f  tests/fixtures/manifests/test-model.json
f  tests/golden/presets.json

# ═══════════════════════════════════════════════════════════════════════════
# PYTHON PACKAGING
# ═══════════════════════════════════════════════════════════════════════════
f python/cibuildwheel.toml
f python/.github/workflows/wheels.yml

# ═══════════════════════════════════════════════════════════════════════════
# NIX
# ═══════════════════════════════════════════════════════════════════════════
f nix/kani.nix

# ═══════════════════════════════════════════════════════════════════════════
# GITHUB WORKFLOWS
# ═══════════════════════════════════════════════════════════════════════════
f .github/workflows/ci.yml
f .github/workflows/build-matrix.yml
f .github/workflows/wheels.yml
f .github/workflows/release.yml
f .github/workflows/kani.yml

# ─── done ─────────────────────────────────────────────────────────────────
echo "✓ Structure created."

# ─── summary ──────────────────────────────────────────────────────────────
printf '\nSummary:\n'
printf '  crates   : %s\n' "$(find "$ROOT/crates" -mindepth 1 -maxdepth 1 -type d | wc -l)"
printf '  files    : %s\n' "$(find "$ROOT" -type f | wc -l)"
printf '  dirs     : %s\n' "$(find "$ROOT" -type d | wc -l)"
printf '  adrs     : %s\n' "$(find "$ROOT/docs/adr" -name '*.md' | wc -l)"
printf '  migrations: %s\n' "$(find "$ROOT/resources/migrations" -name '*.sql' | wc -l)"
