// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # API Key Authentication Middleware
//!
//! ## Purpose
//! Validates the `Authorization: Bearer <key>` header against a configured API key.
//! When `require_api_key` is true, requests without a valid key are rejected with 401.
//!
//! ## What This File Does
//! - `auth_middleware()` — axum middleware function for API key validation
//! - `validate_api_key()` — validates a bearer token against the expected key
//! - `extract_bearer_token()` — extracts the token from the Authorization header
//!
//! ## Dependencies
//! - `axum` — middleware extraction and response
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `http` — header constants
//!
//! ## Output
//! Passes the request through on success, returns 401 on failure.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - auth.rs - function - STATUS`

use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::sync::LazyLock;

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
///
/// ## Function Logging
/// Logs entry and exit: `timestamp - edgeforge-api - auth.rs - extract_bearer_token - STATUS`
pub fn extract_bearer_token(header_value: &str) -> Option<&str> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "auth.rs", "extract_bearer_token");
    let token = header_value.strip_prefix("Bearer ").or_else(|| header_value.strip_prefix("bearer "))?;
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
///
/// ## Function Logging
/// `timestamp - edgeforge-api - auth.rs - validate_api_key - STATUS`
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

/// Axum middleware: enforce API key authentication.
///
/// ## Purpose
/// If `require_api_key` is true, validates the `Authorization: Bearer <key>`
/// header against the expected key. Requests without a valid key are rejected
/// with 401 Unauthorized.
///
/// ## Parameters
/// - `require_api_key`: Whether API key authentication is required.
/// - `expected_key`: The expected API key (None = any key accepted if required).
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
/// ## Function Logging
/// `timestamp - edgeforge-api - auth.rs - auth_middleware - STATUS`
pub async fn auth_middleware(
    require_api_key: bool,
    expected_key: Option<&str>,
    req: Request,
    next: Next,
) -> Response {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "auth.rs", "auth_middleware");

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
}
