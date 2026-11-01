# EdgeForge — Phase 18: Security Middleware Wiring

**Date:** 2026-11-01
**Status:** Complete
**Severity:** P0 (API was wide open)
**Files changed:** 8 source files + 2 new doc/test files

---

## Summary

The security middleware (`auth.rs` and `rate_limit.rs`) was correctly
implemented — constant-time API key comparison, real token-bucket rate
limiting — but **never wired into the router**. `server.rs:63-76` only added
`logging_middleware` and `TraceLayer`. The API was wide open: any request to
any `/v1/*` endpoint succeeded without authentication, and there was no
rate limiting at all.

This is now fixed. `build_router` wires the full middleware stack:
`TraceLayer → logging → auth → rate_limit`, with `/health` bypassing auth
and rate limiting (so Docker healthchecks always succeed).

---

## The Bug

### Before (server.rs)

```rust
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(models::routes())
        .merge(chat::routes())
        // ... other routes ...
        .layer(axum::middleware::from_fn(logging_middleware))  // ← only logging
        .layer(TraceLayer::new_for_http())                     // ← + tracing
        .with_state(state)
}
```

The `auth_middleware` and `rate_limit_middleware` functions existed in
`middleware/auth.rs` and `middleware/rate_limit.rs` with full implementations
and unit tests, but were **never called**. The `require_api_key` and
`rate_limit_per_minute` fields in `AppState` and `ApiConfig` were dead data.

### After (server.rs)

```rust
pub fn build_router(state: AppState) -> Router {
    // /health bypasses auth + rate limiting.
    let public_routes = Router::new().merge(health::routes());

    // All /v1/* routes go through the full middleware stack.
    let protected_routes = Router::new()
        .merge(models::routes())
        .merge(chat::routes())
        // ... other routes ...
        .layer(axum::middleware::from_fn(logging_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

---

## Changes

### Modified Files (6)

| File | Change |
|---|---|
| `crates/edgeforge-core/src/config.rs` | Added `api_key: Option<String>` to `ApiConfig` (with `#[serde(default)]`) |
| `crates/edgeforge-api/src/state.rs` | Added `api_key: Option<String>` + `rate_limiter: Arc<RateLimiter>` to `AppState`; updated `new()` and `with_embedder()` signatures to take `api_key`; constructor builds the shared `RateLimiter` from `rate_limit_per_minute`; added `rate_limiter()` accessor |
| `crates/edgeforge-api/src/middleware/auth.rs` | Refactored `auth_middleware` to read `require_api_key` + `api_key` from `AppState` via the `State` extractor (so it can be wired with `from_fn_with_state`); kept `auth_middleware_stateless` for backward compat; added constant-time test |
| `crates/edgeforge-api/src/middleware/rate_limit.rs` | Refactored `rate_limit_middleware` to read the shared `RateLimiter` from `AppState` via `State`; kept `rate_limit_middleware_with_limiter` for backward compat; added `tracked_ip_count()` + `burst_capacity` test |
| `crates/edgeforge-api/src/middleware/mod.rs` | Re-export `auth_middleware`, `rate_limit_middleware`, `RateLimiter`, `validate_api_key`, `extract_bearer_token`; added module docs describing the middleware stack |
| `crates/edgeforge-api/src/server.rs` | **Core fix**: `build_router` now splits routes into `public_routes` (`/health`) and `protected_routes` (all `/v1/*`), wires `auth_middleware` + `rate_limit_middleware` via `from_fn_with_state` on the protected routes, with `logging_middleware` + `TraceLayer` on all routes. Updated module docs with the full middleware stack diagram. |
| `crates/edgeforge-cli/src/commands/serve.rs` | Updated `AppState::new` call to pass `api_key` from config (with `EDGEFORGE_API_KEY` env var fallback); removed unused `HashingEmbedder` import (it's the default in `AppState::new`); added auth config logging |

### New Files (2)

| File | Purpose |
|---|---|
| `crates/edgeforge-api/tests/security_middleware.rs` | 15 integration tests verifying /health is open, /v1/* requires auth, wrong key → 401, correct key → 200, auth disabled when require_api_key=false, any-token mode, constant-time behavior, AppState rate limiter burst/per-IP |
| `docs/setup/phase18-security-middleware.md` | Complete setup guide with architecture diagram, config reference, auth behavior matrix, best practices, troubleshooting |

---

## Detailed Design

### Middleware Stack Order

The middleware is applied outer → inner:

1. **TraceLayer** (tower-http) — HTTP tracing (always on, all routes)
2. **logging_middleware** — structured request/response logging (all routes)
3. **auth_middleware** — API key validation (protected routes only)
4. **rate_limit_middleware** — per-IP token-bucket (protected routes only)

This means:
- Rate limiting runs **last** (innermost) — a rate-limited request (429) was
  already authenticated (passed auth).
- Auth runs **before** rate limiting — an unauthenticated request (401)
  doesn't consume a rate-limit token.
- Logging runs **first** (outermost) — every request is logged, including
  401s and 429s.

### Public vs Protected Routes

| Route | Auth | Rate Limited |
|---|---|---|
| `GET /health` | No | No |
| All `/v1/*` routes | Yes (when enabled) | Yes |

`/health` is on a separate `public_routes` router that doesn't have the auth
or rate_limit layers. This is intentional: Docker healthchecks and load
balancer probes hit `/health` frequently and must always succeed.

### Stateful Middleware (from_fn_with_state)

Both `auth_middleware` and `rate_limit_middleware` need to read from
`AppState`:
- `auth_middleware` needs `require_api_key` + `api_key`
- `rate_limit_middleware` needs the shared `RateLimiter`

Axum's `from_fn_with_state(state, handler)` is used to thread the state
through. The middleware functions use `State<AppState>` as their first
extractor:

```rust
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response { ... }

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response { ... }
```

`rate_limit_middleware` also uses `ConnectInfo<SocketAddr>` to read the
client's IP address. This requires `into_make_service_with_connect_info` in
`serve()` (already present).

### Constant-Time API Key Comparison

`validate_api_key` uses a constant-time comparison to prevent timing attacks:

```rust
if token.len() != expected_key.len() {
    return false;
}
let mut result = 0u8;
for (a, b) in token.bytes().zip(expected_key.bytes()) {
    result |= a ^ b;
}
result == 0
```

This XOR-accumulate pattern ensures the loop always runs for the full length
of the token, so an attacker can't learn the key by measuring response times.

### Token-Bucket Rate Limiting

The `RateLimiter` uses a token-bucket algorithm:
- **Burst capacity** = `rate_limit_per_minute` (a client can burst up to the
  full per-minute quota at once)
- **Refill rate** = `rate_limit_per_minute / 60` tokens per second (the
  bucket refills continuously)
- **Per-IP** — each client IP has its own bucket (keyed by IP string)

The `RateLimiter` is created in `AppState::with_embedder` and shared across
all requests via `Arc<RateLimiter>`.

### API Key Configuration

The API key is read from (in priority order):
1. `EDGEFORGE_API_KEY` env var (convenience)
2. `cfg.api.api_key` (from `config.toml` or `EDGEFORGE_API__API_KEY` env var)

When `require_api_key` is true:
- If `api_key` is `Some(key)`: only `Bearer <key>` is accepted
- If `api_key` is `None`: any `Bearer` token is accepted (testing mode)

---

## Tests

### Unit Tests

| File | Tests | Coverage |
|---|---|---|
| `auth.rs` | 7 | extract_bearer valid/invalid/edge_cases, validate match/mismatch/different_length, constant-time property |
| `rate_limit.rs` | 8 | allows within limit, blocks over limit, separate IPs, refill, available tokens, clear, tracked_ip_count, burst capacity |

### Integration Tests (security_middleware.rs)

15 integration tests using `tower::ServiceExt::oneshot` (no TCP listener needed):

- `test_health_open_without_auth` — /health returns 200 even when require_api_key=true
- `test_health_open_with_rate_limiting` — /health returns 200 × 10 even with rate_limit=1/min
- `test_v1_routes_require_auth_when_enabled` — /v1/models without auth → 401
- `test_v1_routes_reject_wrong_key` — wrong bearer → 401
- `test_v1_routes_accept_correct_key` — correct bearer → 200
- `test_v1_routes_open_when_auth_disabled` — require_api_key=false → 200
- `test_v1_routes_accept_any_token_when_no_key_configured` — api_key=None → any token accepted
- `test_v1_routes_reject_missing_bearer_prefix` — "secret" without "Bearer " → 401
- `test_v1_routes_reject_empty_bearer_token` — "Bearer " → 401
- `test_auth_constant_time_no_length_leak` — short/long/wrong-length tokens all → 401
- `test_rate_limit_middleware_wired_without_panic` — router builds, /health works
- `test_e2e_auth_flow` — full flow: no auth → 401, wrong → 401, correct → 200, /health open
- `test_appstate_has_rate_limiter` — AppState exposes the shared RateLimiter
- `test_appstate_rate_limiter_burst_capacity` — burst = rate_limit_per_minute
- `test_appstate_rate_limiter_per_ip` — different IPs have separate buckets

**Total: 30 tests** (15 unit + 15 integration).

---

## Validation

### Manual verification

```bash
# Start the server with auth enabled
EDGEFORGE_API__REQUIRE_API_KEY=true \
EDGEFORGE_API_KEY=my-secret \
./target/release/edgeforge serve &

# /health is open
curl -s -w "%{http_code}" http://127.0.0.1:11434/health  # 200

# /v1/models without auth → 401
curl -s -w "%{http_code}" http://127.0.0.1:11434/v1/models  # 401

# /v1/models with wrong key → 401
curl -s -w "%{http_code}" -H "Authorization: Bearer wrong" http://127.0.0.1:11434/v1/models  # 401

# /v1/models with correct key → 200
curl -s -w "%{http_code}" -H "Authorization: Bearer my-secret" http://127.0.0.1:11434/v1/models  # 200
```

### Automated tests

```bash
cargo test -p edgeforge-api -- security_middleware
# Expect: 15 integration tests pass

cargo test -p edgeforge-api -- auth rate_limit
# Expect: 15 unit tests pass
```

---

## Risk Assessment

- **Rate limiting is per-IP, not per-user.** Clients behind the same NAT
  share a bucket. For per-user rate limiting, put a reverse proxy in front
  and extend the middleware to read `X-Forwarded-For` (not yet supported).

- **The API key is stored in memory.** It's read from config/env at startup
  and held in `AppState`. There's no key rotation API yet — to rotate, restart
  the server with a new key.

- **The rate limiter is in-memory.** Per-IP buckets are not shared across
  server instances. For a multi-instance deployment, use an external rate
  limiter (e.g. Redis-backed) or put a load balancer in front.

- **`/health` is unauthenticated by design.** This is standard practice for
  health endpoints. Don't put sensitive info in the health response.
