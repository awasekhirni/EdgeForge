# EdgeForge — Phase 18: Security Middleware Wiring Setup Guide

**Version:** 18.0
**Date:** 2026-11-01

This guide covers the API security middleware: API key authentication,
per-IP rate limiting, and how they're wired into the router.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: Enable API Key Auth](#quick-start-enable-api-key-auth)
3. [Quick Start: Rate Limiting](#quick-start-rate-limiting)
4. [Public vs Protected Routes](#public-vs-protected-routes)
5. [Middleware Stack Order](#middleware-stack-order)
6. [Configuration Reference](#configuration-reference)
7. [API Key Best Practices](#api-key-best-practices)
8. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Incoming Request                         │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  TraceLayer (tower-http) — HTTP tracing                      │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  logging_middleware — structured request/response logging   │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  auth_middleware — API key validation (401 on failure)       │
│  • Reads require_api_key + api_key from AppState             │
│  • Constant-time Bearer token comparison                     │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  rate_limit_middleware — per-IP token-bucket (429 on fail)  │
│  • Reads shared RateLimiter from AppState                    │
│  • Burst capacity = rate_limit_per_minute                    │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  Route Handler (chat, completions, embeddings, etc.)        │
└─────────────────────────────────────────────────────────────┘
```

**Before this fix:** only `logging_middleware` + `TraceLayer` were wired.
`auth_middleware` and `rate_limit_middleware` were dead code — the API was
wide open.

**After this fix:** all four middleware layers are wired. `/health` bypasses
auth + rate limiting; all `/v1/*` routes go through the full stack.

---

## Quick Start: Enable API Key Auth

### Option 1: Config file

```toml
# ~/.config/edgeforge/config.toml

[api]
listen_addr = "127.0.0.1:11434"
require_api_key = true
api_key = "your-secret-api-key-here"
rate_limit_per_minute = 600
```

### Option 2: Environment variable

```bash
export EDGEFORGE_API__REQUIRE_API_KEY=true
export EDGEFORGE_API__API_KEY="your-secret-api-key-here"
export EDGEFORGE_API__RATE_LIMIT_PER_MINUTE=600
# Or use the convenience env var:
export EDGEFORGE_API_KEY="your-secret-api-key-here"

./target/release/edgeforge serve
```

### Verify auth is enforced

```bash
# Without auth → 401
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:11434/v1/models
# 401

# With wrong key → 401
curl -s -o /dev/null -w "%{http_code}\n" \
  -H "Authorization: Bearer wrong-key" \
  http://127.0.0.1:11434/v1/models
# 401

# With correct key → 200
curl -s -o /dev/null -w "%{http_code}\n" \
  -H "Authorization: Bearer your-secret-api-key-here" \
  http://127.0.0.1:11434/v1/models
# 200

# /health is always open
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:11434/health
# 200
```

---

## Quick Start: Rate Limiting

Rate limiting is **always enabled** (default: 600 req/min per IP). The token
bucket has a burst capacity equal to the per-minute limit, so a client can
briefly burst up to the full quota.

### Configure the rate limit

```toml
[api]
rate_limit_per_minute = 100  # 100 req/min per IP
```

### Verify rate limiting

```bash
# Send 101 requests rapidly → the 101st returns 429
for i in $(seq 1 101); do
  status=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer your-key" \
    http://127.0.0.1:11434/v1/models)
  echo "Request $i: $status"
done
# Requests 1-100: 200
# Request 101: 429 (Too Many Requests)
```

### Rate limit response

When rate-limited, the API returns:

```
HTTP/1.1 429 Too Many Requests
```

The rate limiter uses a token-bucket algorithm:
- **Burst capacity** = `rate_limit_per_minute` (a client can burst up to the
  full per-minute quota at once)
- **Refill rate** = `rate_limit_per_minute / 60` tokens per second (the
  bucket refills continuously)
- **Per-IP** — each client IP has its own bucket

---

## Public vs Protected Routes

| Route | Auth Required | Rate Limited | Purpose |
|---|---|---|---|
| `GET /health` | No | No | Liveness probe (Docker healthcheck) |
| `GET /v1/models` | Yes (when enabled) | Yes | List models |
| `GET /v1/models/:id` | Yes (when enabled) | Yes | Get a model |
| `POST /v1/chat/completions` | Yes (when enabled) | Yes | Chat completion |
| `POST /v1/completions` | Yes (when enabled) | Yes | Text completion |
| `POST /v1/embeddings` | Yes (when enabled) | Yes | Generate embeddings |
| `POST /v1/embeddings/search` | Yes (when enabled) | Yes | RAG similarity search |
| `POST /v1/files` | Yes (when enabled) | Yes | RAG file upload |
| `GET/POST /v1/webhooks` | Yes (when enabled) | Yes | Webhook management |

**Why is `/health` public?** Docker healthchecks and load balancer probes
hit `/health` frequently (often every 5–10 seconds). If `/health` required
auth or was rate-limited, health probes would fail and the container would
be marked unhealthy.

---

## Middleware Stack Order

The middleware is applied in this order (outer → inner):

1. **TraceLayer** (tower-http) — HTTP tracing (always on)
2. **logging_middleware** — structured request/response logging (always on)
3. **auth_middleware** — API key validation (on when `require_api_key=true`)
4. **rate_limit_middleware** — per-IP token-bucket (always on)

This means:
- Rate limiting runs **last** (innermost) — a rate-limited request (429) was
  already authenticated (passed auth).
- Auth runs **before** rate limiting — an unauthenticated request (401)
  doesn't consume a rate-limit token.
- Logging runs **first** (outermost) — every request is logged, including
  401s and 429s.

### Wiring code

```rust
// From server.rs::build_router
let protected_routes = Router::new()
    .merge(models::routes())
    .merge(chat::routes())
    // ... other /v1/* routes ...
    .layer(axum::middleware::from_fn(logging_middleware))
    .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
    .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));
```

---

## Configuration Reference

### `[api]` section

| Field | Type | Default | Purpose |
|---|---|---|---|
| `listen_addr` | String | `"127.0.0.1:11434"` | Listen address |
| `require_api_key` | bool | `false` | Whether API key auth is required |
| `api_key` | Option<String> | `None` | The expected API key |
| `rate_limit_per_minute` | u32 | `600` | Per-IP rate limit (req/min) |

### Environment variables

| Variable | Purpose |
|---|---|
| `EDGEFORGE_API__REQUIRE_API_KEY` | Override `require_api_key` |
| `EDGEFORGE_API__API_KEY` | Override `api_key` |
| `EDGEFORGE_API__RATE_LIMIT_PER_MINUTE` | Override `rate_limit_per_minute` |
| `EDGEFORGE_API_KEY` | Convenience: set the API key (takes precedence over config) |

### Auth behavior matrix

| `require_api_key` | `api_key` | Behavior |
|---|---|---|
| `false` | (ignored) | No auth — all `/v1/*` routes are open |
| `true` | `None` | Any `Bearer` token accepted (testing mode) |
| `true` | `Some(key)` | Only `Bearer <key>` accepted |

---

## API Key Best Practices

### 1. Use a long, random key

```bash
# Generate a 32-byte random key (base64-encoded)
openssl rand -base64 32
# e.g.: K7mRj8L2nQ9vX4wY1aZ6bC3dE5fG8hJ0kM2nP4qR6sT=
```

### 2. Set the key via environment variable in production

```bash
# In your systemd unit, Docker Compose, or Kubernetes manifest:
Environment=EDGEFORGE_API_KEY=K7mRj8L2nQ9vX4wY1aZ6bC3dE5fG8hJ0kM2nP4qR6sT=
Environment=EDGEFORGE_API__REQUIRE_API_KEY=true
```

### 3. Don't commit the key to version control

Use a config file that's gitignored, or (better) use environment variables.

### 4. The comparison is constant-time

`validate_api_key` uses a constant-time comparison to prevent timing attacks:

```rust
let mut result = 0u8;
for (a, b) in token.bytes().zip(expected_key.bytes()) {
    result |= a ^ b;
}
result == 0
```

This means an attacker can't learn the key by measuring response times.

### 5. Rate limiting is per-IP

Each client IP has its own token bucket. This means:
- A single client can't exceed `rate_limit_per_minute`
- Multiple clients behind the same NAT share a bucket (by IP)
- For per-user rate limiting, put a reverse proxy (nginx, Caddy) in front
  and use `X-Forwarded-For` (not yet supported by the middleware — the
  limiter uses the direct socket address)

---

## Troubleshooting

### "401 Unauthorized" on all /v1/* routes

Check that you're sending the correct `Authorization: Bearer <key>` header:

```bash
curl -v -H "Authorization: Bearer your-key" http://127.0.0.1:11434/v1/models
```

Common mistakes:
- Missing `Bearer ` prefix → `Authorization: your-key` (wrong)
- Wrong case → `Authorization: bearer your-key` (actually works, but use `Bearer`)
- Empty token → `Authorization: Bearer ` (rejected)

### "429 Too Many Requests"

You've exceeded the rate limit. Wait for the bucket to refill (continuously,
at `rate_limit_per_minute / 60` tokens per second) or increase the limit:

```toml
[api]
rate_limit_per_minute = 1200  # double the default
```

### "/health returns 401"

This should never happen — `/health` is always public. If you see this:
- You're running an old build. Rebuild: `cargo build --release`
- You've modified `build_router` to put `/health` under the auth layer

### "API is wide open despite require_api_key=true"

This was a **Phase 17 bug** (auth middleware was dead code). It is fixed:
`build_router` now wires `auth_middleware` via `from_fn_with_state`. Update
to the latest build:

```bash
cargo build --release
```

### Auth works but rate limiting doesn't

Verify the server is using `into_make_service_with_connect_info` (required
for the `ConnectInfo<SocketAddr>` extractor). This is set in `serve()`:

```rust
axum::serve(
    listener,
    app.into_make_service_with_connect_info::<SocketAddr>(),
)
```

If you're using a custom server setup, make sure to use
`into_make_service_with_connect_info` instead of `into_make_service`.

### How do I disable auth for testing?

Set `require_api_key = false` in your config, or don't set the
`EDGEFORGE_API__REQUIRE_API_KEY` env var. All `/v1/*` routes will be open.

### How do I disable rate limiting for testing?

Set `rate_limit_per_minute` to a very high value:

```toml
[api]
rate_limit_per_minute = 1000000  # effectively unlimited
```

There's no "off" switch — the rate limiter is always wired. This is
intentional: a public API should always have some rate limit.
