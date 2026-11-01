// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # X-EdgeForge-Verification Header Middleware (ADR-018)
//!
//! ## Purpose
//! Injects the `X-EdgeForge-Verification` header into SSE responses to indicate
//! the verification status of each token. Used for Tier 2/3 crowd verification.
//!
//! ## What This File Does
//! - `HEADER_NAME` — the header name constant
//! - `set_verification_header()` — set the header on a response
//! - `VerificationHeaderValue` — enum for the header value
//! - `verification_middleware()` — axum middleware for header injection
//!
//! ## Dependencies
//! - `axum` — response and header manipulation
//! - `edgeforge-crowd` — VerificationOutcome enum
//! - `edgeforge-observability` — FunctionGuard for call logging
//!
//! ## Output
//! HTTP responses with the `X-EdgeForge-Verification` header set.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - verification.rs - function - STATUS`

use axum::{
    http::{HeaderMap, HeaderValue},
    response::Response,
};
use edgeforge_crowd::VerificationOutcome;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The verification header name.
pub const HEADER_NAME: &str = "X-EdgeForge-Verification";

/// The possible values for the verification header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationHeaderValue {
    /// Tier 1 / Local — no verification needed.
    NotRequired,
    /// Tier 2/3 — verification passed within the 50ms deadline.
    Verified,
    /// Tier 2/3 — verification pending at emit time. A correction chunk may follow.
    Pending,
    /// This token corrects a previously-emitted Pending token that failed verification.
    Corrected,
}

impl VerificationHeaderValue {
    /// Convert to the header string value.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Verified => "verified",
            Self::Pending => "pending",
            Self::Corrected => "corrected",
        }
    }

    /// Convert from a VerificationOutcome.
    pub fn from_outcome(outcome: VerificationOutcome) -> Self {
        match outcome {
            VerificationOutcome::NotRequired => Self::NotRequired,
            VerificationOutcome::Verified => Self::Verified,
            VerificationOutcome::Pending => Self::Pending,
            VerificationOutcome::Corrected => Self::Corrected,
        }
    }
}

impl std::fmt::Display for VerificationHeaderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Set the verification header on a response's headers.
///
/// ## Purpose
/// Adds the `X-EdgeForge-Verification` header to the response headers,
/// indicating the verification status of the token(s) in the response.
///
/// ## Parameters
/// - `headers`: The response headers to modify.
/// - `outcome`: The verification outcome to set.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - verification.rs - set_verification_header - STATUS`
pub fn set_verification_header(headers: &mut HeaderMap, outcome: VerificationOutcome) {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-api",
        "verification.rs",
        "set_verification_header",
    );
    let value = VerificationHeaderValue::from_outcome(outcome);
    if let Ok(hv) = HeaderValue::from_str(value.as_str()) {
        headers.insert(HEADER_NAME, hv);
    }
}

/// Set the verification header on an axum Response.
///
/// ## Purpose
/// Convenience function that extracts the headers from the response and
/// calls `set_verification_header`.
///
/// ## Parameters
/// - `response`: The response to modify.
/// - `outcome`: The verification outcome to set.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - verification.rs - set_verification_header_on_response - STATUS`
pub fn set_verification_header_on_response(response: &mut Response, outcome: VerificationOutcome) {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-api",
        "verification.rs",
        "set_verification_header_on_response",
    );
    set_verification_header(response.headers_mut(), outcome);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_verification_header_value_from_outcome() {
        assert_eq!(
            VerificationHeaderValue::from_outcome(VerificationOutcome::NotRequired),
            VerificationHeaderValue::NotRequired
        );
        assert_eq!(
            VerificationHeaderValue::from_outcome(VerificationOutcome::Verified),
            VerificationHeaderValue::Verified
        );
        assert_eq!(
            VerificationHeaderValue::from_outcome(VerificationOutcome::Pending),
            VerificationHeaderValue::Pending
        );
        assert_eq!(
            VerificationHeaderValue::from_outcome(VerificationOutcome::Corrected),
            VerificationHeaderValue::Corrected
        );
    }

    #[test]
    fn test_verification_header_value_as_str() {
        assert_eq!(
            VerificationHeaderValue::NotRequired.as_str(),
            "not_required"
        );
        assert_eq!(VerificationHeaderValue::Verified.as_str(), "verified");
        assert_eq!(VerificationHeaderValue::Pending.as_str(), "pending");
        assert_eq!(VerificationHeaderValue::Corrected.as_str(), "corrected");
    }

    #[test]
    fn test_set_verification_header() {
        let mut headers = HeaderMap::new();
        set_verification_header(&mut headers, VerificationOutcome::Verified);
        assert_eq!(
            headers.get(HEADER_NAME).unwrap().to_str().unwrap(),
            "verified"
        );
    }

    #[test]
    fn test_set_verification_header_pending() {
        let mut headers = HeaderMap::new();
        set_verification_header(&mut headers, VerificationOutcome::Pending);
        assert_eq!(
            headers.get(HEADER_NAME).unwrap().to_str().unwrap(),
            "pending"
        );
    }

    #[test]
    fn test_set_verification_header_not_required() {
        let mut headers = HeaderMap::new();
        set_verification_header(&mut headers, VerificationOutcome::NotRequired);
        assert_eq!(
            headers.get(HEADER_NAME).unwrap().to_str().unwrap(),
            "not_required"
        );
    }
}
