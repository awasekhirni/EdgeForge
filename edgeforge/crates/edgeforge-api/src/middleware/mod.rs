//! # Axum Middleware
//!
//! ## Purpose
//! Centralized error handling, rate limiting, authentication, verification
//! header injection, and structured logging.
//!
//! ## Modules
//! - `auth` — API key authentication (Authorization: Bearer header).
//!   Wired into the router via `from_fn_with_state(state, auth_middleware)`.
//! - `error` — centralized error → JSON response mapping
//! - `logging` — structured request/response logging.
//!   Wired into the router via `from_fn(logging_middleware)`.
//! - `rate_limit` — per-IP token-bucket rate limiting.
//!   Wired into the router via `from_fn_with_state(state, rate_limit_middleware)`.
//! - `verification` — X-EdgeForge-Verification header injection (ADR-018)
//!
//! ## Middleware Stack (outer → inner, as applied by `server::build_router`)
//! 1. `TraceLayer` (tower-http) — HTTP tracing
//! 2. `logging_middleware` — structured request/response logging
//! 3. `auth_middleware` — API key validation (401 on failure)
//! 4. `rate_limit_middleware` — per-IP token-bucket (429 on failure)
//!
//! The `/health` endpoint bypasses auth + rate limiting (mounted on a
//! separate router). All `/v1/*` routes go through the full stack.

pub mod auth;
pub mod error;
pub mod logging;
pub mod rate_limit;
pub mod verification;

pub use error::ApiError;
pub use rate_limit::{rate_limit_middleware, rate_limit_middleware_with_limiter, RateLimiter};
pub use auth::{auth_middleware, auth_middleware_stateless, extract_bearer_token, validate_api_key};
