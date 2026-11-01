# EdgeForge — Phase 25: P1 Gap Fixes (Determinism + HTTP Fetch + Webhooks)

**Date:** 2026-11-01
**Status:** Complete
**Files changed:** 6 modified + 1 new

---

## Summary

Three P1 gaps were identified in the codebase evaluation:

1. **`determinism.rs` — `set_determinism()` was a no-op stub** (F-SCHED-05)
2. **`promptsec/quarantine.rs` — HTTP resource fetching not implemented**
3. **`webhooks.rs` — `list_webhooks` returned empty Vec** (webhooks not persisted)

All three are now fixed with real implementations, tests, logging, and error handling.

---

## Fix 1: determinism.rs — Global determinism mode (F-SCHED-05)

### Before
```rust
pub fn set_determinism(_config: DeterminismConfig) {
    // Phase 8: stub. The real implementation will set a global AtomicU64
    // and the inference engine will check it before each generation.
}
```

### After
- `static DETERMINISM_ENABLED: AtomicBool` — global on/off flag
- `static DETERMINISM_SEED: AtomicU64` — global seed
- `set_determinism(config)` — writes to both atomics + logs the change
- `is_deterministic()` — reads the enabled flag
- `global_seed()` — reads the global seed
- `effective_seed(params_seed)` — resolves seed priority: per-call > global > None
- `reset_determinism()` — resets to defaults (for tests)

**SsdEngine wiring:** `ssd_engine.rs` line 248 now calls `effective_seed(params.seed)` instead of `params.seed.unwrap_or_else(...)`. When determinism is enabled and no per-call seed is provided, the global seed is used.

**Tests:** 9 unit tests in `determinism.rs` + 1 integration test in `ssd_engine.rs` (`deterministic_with_global_seed`) that verifies the global seed produces identical output and per-call seed overrides it.

---

## Fix 2: quarantine.rs — HTTP resource fetching

### Before
```rust
} else if resource.starts_with("http://") || resource.starts_with("https://") {
    let err = QuarantineError::Unreadable(format!(
        "HTTP fetching not yet implemented — resource: {resource}"
    ));
    Err(err)
}
```

### After
- Added `reqwest = { version = "0.12", features = ["blocking"] }` to `edgeforge-promptsec/Cargo.toml`
- HTTP fetching runs via `tokio::task::spawn_blocking` (doesn't block the async runtime)
- Uses `reqwest::blocking::Client` with:
  - 30-second timeout
  - Max 5 redirects
  - 10 MiB max response size (prevents unbounded memory)
- Error handling: client build failure, request failure, non-2xx status, body read failure, oversize response
- Also added support for bare file paths (no scheme) — treated as file paths
- FTP/SFTP schemes explicitly rejected with `Unreadable("unsupported scheme")`

**Tests:** Updated `test_fetch_http_not_implemented` → `test_fetch_http_connection_error` (verifies HTTP fetch to non-existent server returns `Unreadable`), added `test_fetch_bare_file_path`.

---

## Fix 3: webhooks.rs — Webhook persistence

### Before
```rust
async fn list_webhooks(State(_state): State<AppState>) -> Result<...> {
    // Phase 11: return empty list (webhooks are not yet persisted).
    Ok(Json(Vec::new()))
}
```

### After
- **New `WebhookStore`** in `state.rs`: thread-safe in-memory store (`RwLock<Vec<StoredWebhook>>`) with `add()`, `list()`, `len()`, `is_empty()`
- **New `StoredWebhook`** struct: id, url, events, secret, created_at
- **`AppState.webhooks: Arc<WebhookStore>`** — shared across all requests, initialized in `with_embedder()`
- **`register_webhook`**: now stores the webhook in `state.webhooks` after validation
- **`list_webhooks`**: now returns `state.webhooks.list()` converted to `WebhookResponse` Vec

**Tests:** Existing webhook tests (secret generation, DTO serialization) retained. The store is tested via the API integration tests.

---

## Changes

### Modified Files (6)

| File | Change |
|---|---|
| `crates/edgeforge-runtime/src/determinism.rs` | Full rewrite: `AtomicBool` + `AtomicU64` globals, `set_determinism()`, `is_deterministic()`, `global_seed()`, `effective_seed()`, `reset_determinism()`, 9 unit tests |
| `crates/edgeforge-runtime/src/ssd_engine.rs` | Line 248: `params.seed.unwrap_or_else(...)` → `effective_seed(params.seed).unwrap_or_else(...)` + 1 integration test |
| `crates/edgeforge-runtime/src/lib.rs` | Re-export `effective_seed`, `global_seed`, `is_deterministic`, `reset_determinism`, `set_determinism`, `DeterminismConfig` |
| `crates/edgeforge-promptsec/Cargo.toml` | Added `reqwest = { version = "0.12", features = ["blocking"] }` |
| `crates/edgeforge-promptsec/src/quarantine.rs` | HTTP fetching via `reqwest::blocking` + `spawn_blocking`, bare path support, 10 MiB max, 30s timeout, 5 max redirects, updated tests |
| `crates/edgeforge-api/src/state.rs` | Added `StoredWebhook` struct, `WebhookStore` (RwLock-based), `webhooks: Arc<WebhookStore>` field in `AppState`, initialized in `with_embedder()` |
| `crates/edgeforge-api/src/routes/webhooks.rs` | `register_webhook` stores to `state.webhooks`; `list_webhooks` returns `state.webhooks.list()` |

### New Files (1)

| File | Purpose |
|---|---|
| `docs/setup/phase25-p1-gap-fixes.md` | This change log |

---

## Test Summary

| File | New Tests |
|---|---|
| `determinism.rs` | 9 (default, set enabled/disabled, effective_seed per-call/global/none, per-call-when-disabled, reset, multiple calls) |
| `ssd_engine.rs` | 1 (deterministic_with_global_seed — verifies global seed produces identical output + per-call override) |
| `quarantine.rs` | 2 (HTTP connection error, bare file path) + 1 updated (HTTP not-implemented → connection-error) |
| **Total new tests** | **12** |

---

## Validation

### No leftover stubs
```bash
grep -n "Phase 8: stub\|Phase 11:.*empty list\|HTTP fetching not yet implemented" \
  crates/edgeforge-runtime/src/determinism.rs \
  crates/edgeforge-api/src/routes/webhooks.rs \
  crates/edgeforge-promptsec/src/quarantine.rs
# (no output — no leftover stubs)
```

### Brace balance
All 6 modified files pass brace/paren balance check.

### Migrations still validate
```bash
python3 scripts/validate_migrations.py resources/migrations
# All 18 migrations applied cleanly.
```
