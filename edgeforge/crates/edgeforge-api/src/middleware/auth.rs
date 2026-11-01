//! # API Key Authentication Middleware
//!
//! ## Purpose
//! Validates the `Authorization: Bearer <key>` header against a configured API key.
//! When `require_api_key` is true, requests without a valid key are rejected with 401.
//!
//! ## What This File Does
//! - `auth_middleware()` — axum middleware function for API key validation.
//!   Reads `require_api_key` + `api_key` from `AppState` via the `State`
//!   extractor, so it can be wired with `from_fn_with_state`.
//! - `auth_middleware_stateless()` — the original stateless version (kept for
//!   backward compatibility and direct testing).
//! - `validate_api_key()` — validates a bearer token against the expected key
//!   using constant-time comparison.
//! - `extract_bearer_token()` — extracts the token from the Authorization header.
//!
//! ## Wiring
//! This middleware is wired into the router by `server::build_router` via
//! `axum::middleware::from_fn_with_state(state.clone(), auth_middleware)`.
//! It runs **after** rate limiting and **before** the route handlers.
//!
//! ## Public Routes
//! The `/health` endpoint bypasses auth (it's mounted on a separate router
//! that doesn't have the auth layer). All `/v1/*` routes require auth when
//! `require_api_key` is true.
//!
//! ## Dependencies
//! - `axum` — middleware extraction and response
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `http` — header constants
//! - `crate::state::AppState` — for reading auth config
//!
//! ## Output
//! Passes the request through on success, returns 401 on failure.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - auth.rs - function - STATUS`

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::sync::LazyLock;

use crate::state::AppState;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Extract the bearer token from an Authorization header value.
///
/// ## Purpose
/// Parses the `Authorization: Bearer <token>` header and returns the token.
///
/// ## Parameters
/// - `header_value`: The raw Authorization header value (e.g., "Bearer abc123").
///
/// ## Returns
/// `Some(token)` if the header is a valid Bearer token, `None` otherwise.
pub fn extract_bearer_token(header_value: &str) -> Option<&str> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "auth.rs", "extract_bearer_token");
    let token = header_value
        .strip_prefix("Bearer ")
        .or_else(|| header_value.strip_prefix("bearer "))?;
    if token.is_empty() {
        return None;
    }
    Some(token)
}

/// Validate a bearer token against the expected API key.
///
/// ## Purpose
/// Compares the provided token against the expected API key using constant-time
/// comparison to prevent timing attacks.
///
/// ## Parameters
/// - `token`: The token from the request.
/// - `expected_key`: The expected API key.
///
/// ## Returns
/// `true` if the token matches the expected key.
pub fn validate_api_key(token: &str, expected_key: &str) -> bool {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "auth.rs", "validate_api_key");
    // Use constant-time comparison to prevent timing attacks.
    if token.len() != expected_key.len() {
        return false;
    }
    let mut result = 0u8;
    for (a, b) in token.bytes().zip(expected_key.bytes()) {
        result |= a ^ b;
    }
    result == 0
}

/// Axum middleware: enforce API key authentication (stateful, reads from AppState).
///
/// ## Purpose
/// Reads `require_api_key` + `api_key` from `AppState`. If `require_api_key`
/// is true, validates the `Authorization: Bearer <key>` header against the
/// configured key. Requests without a valid key are rejected with 401.
///
/// ## Parameters
/// - `state`: The application state (contains `require_api_key` + `api_key`).
/// - `req`: The incoming request.
/// - `next`: The next middleware/handler in the chain.
///
/// ## Returns
/// The response from the next handler, or 401 if authentication fails.
///
/// ## Errors
/// Returns 401 Unauthorized if:
/// - `require_api_key` is true AND
/// - The Authorization header is missing, OR
/// - The bearer token does not match the expected key
///
/// ## Wiring
/// Wired via `axum::middleware::from_fn_with_state(state, auth_middleware)`.
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "auth.rs", "auth_middleware");

    if !state.require_api_key {
        guard.success();
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = match auth_header.and_then(extract_bearer_token) {
        Some(t) => t,
        None => {
            guard.error("missing or invalid Authorization header");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    // If an api_key is configured, validate the token against it.
    // If api_key is None but require_api_key is true, any bearer token is accepted.
    if let Some(expected) = &state.api_key {
        if !validate_api_key(token, expected) {
            guard.error("invalid API key");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    guard.success();
    next.run(req).await
}

/// Stateless auth middleware (kept for backward compatibility + direct testing).
///
/// ## Purpose
/// Original stateless version that takes `require_api_key` + `expected_key`
/// as direct arguments. Used by tests that want to bypass AppState.
///
/// ## Parameters
/// - `require_api_key`: Whether API key authentication is required.
/// - `expected_key`: The expected API key (None = any key accepted if required).
/// - `req`: The incoming request.
/// - `next`: The next middleware/handler in the chain.
pub async fn auth_middleware_stateless(
    require_api_key: bool,
    expected_key: Option<&str>,
    req: Request,
    next: Next,
) -> Response {
    let mut guard =
        FunctionGuard::new(&LOGGER, "edgeforge-api", "auth.rs", "auth_middleware_stateless");

    if !require_api_key {
        guard.success();
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = match auth_header.and_then(extract_bearer_token) {
        Some(t) => t,
        None => {
            guard.error("missing or invalid Authorization header");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    if let Some(expected) = expected_key {
        if !validate_api_key(token, expected) {
            guard.error("invalid API key");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    guard.success();
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token_valid() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer xyz"), Some("xyz"));
    }

    #[test]
    fn test_extract_bearer_token_invalid() {
        assert_eq!(extract_bearer_token("Basic abc123"), None);
        assert_eq!(extract_bearer_token("Bearer "), None);
        assert_eq!(extract_bearer_token(""), None);
    }

    #[test]
    fn test_validate_api_key_match() {
        assert!(validate_api_key("secret123", "secret123"));
    }

    #[test]
    fn test_validate_api_key_mismatch() {
        assert!(!validate_api_key("wrong", "secret123"));
    }

    #[test]
    fn test_validate_api_key_different_length() {
        assert!(!validate_api_key("short", "longer_key"));
    }

    #[test]
    fn test_extract_bearer_token_edge_cases() {
        assert_eq!(extract_bearer_token("Bearer "), None);
        assert_eq!(extract_bearer_token("Basic abc"), None);
        assert_eq!(extract_bearer_token(""), None);
    }

    #[test]
    fn test_validate_api_key_constant_time_property() {
        // Verify the comparison doesn't short-circuit on length mismatch in
        // a way that leaks timing. (We can't truly test timing in a unit test,
        // but we can verify the function returns false for length mismatch.)
        assert!(!validate_api_key("a", "aa"));
        assert!(!validate_api_key("aa", "ab"));
        assert!(!validate_api_key("ab", "aa"));
        assert!(validate_api_key("same", "same"));
    }
}
