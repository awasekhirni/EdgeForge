# EdgeForge — Phase 24: CLI Bench + Tests Implementation

**Date:** 2026-11-01
**Status:** Complete
**Feature:** F-MON-01 (CLI Benchmark) + CLI Test Coverage
**Files changed:** 4 modified + 2 new

---

## Summary

Two P0 gaps were identified in the CLI crate:

1. **CLI `bench` command was a stub** — `run()` printed `"bench: stub — Phase 3"` and returned `Ok(())`. No actual benchmark was run.
2. **CLI had zero tests** — No `#[test]` or `#[tokio::test]` anywhere in the crate.

Both are now fixed:
- `bench::run()` loads a model from the registry, creates an `SsdEngine`, runs `run_benchmark()`, and prints a formatted results report
- 10 integration tests cover bench (success + invalid ID + not found), config (JSON + TOML), model (list empty + list after import + verify existing + verify not found), and serve (config validation)

---

## Changes

### Modified Files (4)

| File | Change |
|---|---|
| `crates/edgeforge-cli/Cargo.toml` | Added `edgeforge-crowd` + `edgeforge-security` deps (needed for `ExecutionMode` + `ModelManifest`); added `[lib]` section with `edgeforge_cli` lib name; added `tempfile` + `tokio` dev-deps |
| `crates/edgeforge-cli/src/main.rs` | Changed `mod commands` to `use edgeforge_cli::commands` (uses the new lib) |
| `crates/edgeforge-cli/src/commands/bench.rs` | Full rewrite: opens DB, looks up model by ID, builds `ModelManifest`, creates `SsdEngine`, runs `run_benchmark()`, prints formatted results report with tok/s, first-token latency, SSD metrics |
| `crates/edgeforge-cli/src/commands/mod.rs` | Already had `pub` modules — no changes needed |

### New Files (2)

| File | Purpose |
|---|---|
| `crates/edgeforge-cli/src/lib.rs` | Library entry point that re-exports `commands` for integration tests |
| `crates/edgeforge-cli/tests/cli_integration.rs` | 10 integration tests for all CLI subcommands |

---

## Detailed Design

### bench::run() — Real Benchmark Execution

The `bench::run()` function now:

1. **Opens the SQLite database** at `cfg.paths.data_dir/edgeforge.db` and applies migrations (same pattern as `model::run()` and `serve::run()`)
2. **Parses the model ID** from `args.model` (must be a valid ULID)
3. **Looks up the model** in the registry via `Models::get(model_id)`
4. **Builds a `ModelManifest`** from the registry row (name, publisher, license, sha256, precision, arch, params)
5. **Creates an `SsdEngine`** and loads the model via `engine.load(&manifest, ExecutionMode::Local)`
6. **Runs `run_benchmark()`** from `edgeforge-orchestration::benchmark` with the configured warmup + measured token counts
7. **Prints a formatted report** with tokens/sec, first-token latency, total elapsed, cold-cache flag, SSD read MB/s, SSD temperature

### Error Handling

- Invalid model ID (not a valid ULID) → `anyhow::anyhow!("invalid model ID")`
- Model not found in registry → `anyhow::anyhow!("model not found")`
- Model load failure → `anyhow::anyhow!("failed to load model: {e}")`
- Benchmark failure → `anyhow::anyhow!("benchmark failed: {e}")`
- DB open/migration failure → propagated `anyhow::Error`

### Logging

All major steps are logged via `tracing::info!`:
- Benchmark start (model ID, warmup, tokens)
- Model found in registry (ID, name, precision)
- Model loaded, benchmark starting
- Benchmark complete (tok/s, first_token_ms, elapsed_ms)

### Integration Tests (10)

| Test | What it verifies |
|---|---|
| `test_bench_runs_benchmark` | bench::run() succeeds with a valid model — runs a real SsdEngine benchmark |
| `test_bench_invalid_model_id` | bench::run() fails with an invalid ULID |
| `test_bench_model_not_found` | bench::run() fails when the model ID doesn't exist in the registry |
| `test_config_json_output` | config --json succeeds and prints JSON |
| `test_config_toml_output` | config (TOML) succeeds and prints TOML |
| `test_model_list_empty` | model list succeeds on an empty DB |
| `test_model_list_after_import` | model list succeeds after a model is imported |
| `test_model_verify_existing` | model verify succeeds for an existing model |
| `test_model_verify_not_found` | model verify fails for a non-existent model |
| `test_serve_config_validation` | config validates correctly (serve uses the same config) |

All tests use a temp-dir SQLite database to avoid touching the user's real data directory.

---

## Validation

### No leftover stubs

```bash
grep -n "stub\|Phase 3" crates/edgeforge-cli/src/commands/bench.rs
# (no output — no leftover stubs)
```

### bench.rs imports real modules

```bash
grep "use edgeforge" crates/edgeforge-cli/src/commands/bench.rs
# use edgeforge_core::Config;
# use edgeforge_crowd::ExecutionMode;
# use edgeforge_orchestration::benchmark::{run_benchmark, BenchmarkConfig};
# use edgeforge_orchestration::Models;
# use edgeforge_runtime::{InferenceEngine, SsdEngine};
# use edgeforge_security::{ModelArch, ModelManifest};
```

### CLI now has 10 tests (was 0)

```bash
grep -rc "#\[test\]\|#\[tokio::test\]" crates/edgeforge-cli/tests/cli_integration.rs
# 10
```

### Migrations still validate

```bash
python3 scripts/validate_migrations.py resources/migrations
# All 18 migrations applied cleanly.
```
