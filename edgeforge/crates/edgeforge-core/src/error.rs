//! # Error Taxonomy
//!
//! ## Purpose
//! Defines the unified `EdgeForgeError` enum that all crates use for error
//! handling. Every error variant maps to an HTTP status code for the API layer.
//!
//! ## What This File Does
//! - Defines `EdgeForgeError` with variants for every layer (L1–L5)
//! - Implements `thiserror::Error` for automatic `Display` and `From` impls
//! - Provides `http_status()` for API middleware error → JSON mapping
//!
//! ## Dependencies
//! - `thiserror` — for `#[derive(Error)]`
//! - `std::path::PathBuf` — for I/O error paths
//!
//! ## Output
//! `EdgeForgeError` enum with `http_status() -> u16` method.
//!
//! ## Error → HTTP Status Mapping
//! | Variant | HTTP Status |
//! |---|---|
//! | InvalidRequest, InvalidPreset | 400 |
//! | Unauthorized, MtlsHandshakeFailed | 401 |
//! | RateLimited | 429 |
//! | ModelNotFound, JobNotFound | 404 |
//! | InvalidJobState | 409 |
//! | LicenseForbidden | 403 |
//! | NotImplemented | 501 |
//! | Others | 500 |

use std::path::PathBuf;
use thiserror::Error;
/// Type alias for `Result`.
pub type Result<T, E = EdgeForgeError> = std::result::Result<T, E>;
#[derive(Debug, Error)]
/// `EdgeForgeError` enumeration — see module documentation for variants.
pub enum EdgeForgeError {
    #[error("manifest signature invalid: {0}")] ManifestSignatureInvalid(String),
    #[error("integrity mismatch: expected {expected}, got {actual}")] IntegrityMismatch { expected: String, actual: String },
    #[error("license forbidden: {license} for {use_case}")] LicenseForbidden { license: String, use_case: String },
    #[error("sandbox denied: {0}")] SandboxDenied(String),
    #[error("mesh error: {0}")] Mesh(String),
    #[error("peer unreachable: {peer}")] PeerUnreachable { peer: String },
    #[error("mTLS failed: {0}")] MtlsHandshakeFailed(String),
    #[error("I/O error on {path:?}: {source}")] Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("insufficient SSD: need {needed}, have {have}")] InsufficientSsdSpace { needed: u64, have: u64 },
    #[error("SSD thermal critical: {temp_celsius}°C")] SsdThermalCritical { temp_celsius: f32 },
    #[error("model load failed: {model}: {reason}")] ModelLoadFailed { model: String, reason: String },
    #[error("engine error: {0}")] Engine(String),
    #[error("model not found: {0}")] ModelNotFound(String),
    #[error("job not found: {0}")] JobNotFound(String),
    #[error("invalid preset: {0}")] InvalidPreset(String),
    #[error("invalid job state: {job} is {state}")] InvalidJobState { job: String, state: String },
    #[error("invalid request: {0}")] InvalidRequest(String),
    #[error("rate limited: {0}")] RateLimited(String),
    #[error("unauthorized: {0}")] Unauthorized(String),
    #[error("database error: {0}")] Database(String),
    #[error("config error: {0}")] Config(String),
    #[error("not implemented: {0}")] NotImplemented(&'static str),
}
impl EdgeForgeError {
    /// Http status — see module documentation for details.
    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidRequest(_) | Self::InvalidPreset(_) => 400,
            Self::Unauthorized(_) | Self::MtlsHandshakeFailed(_) => 401,
            Self::RateLimited(_) => 429,
            Self::ModelNotFound(_) | Self::JobNotFound(_) => 404,
            Self::InvalidJobState { .. } => 409,
            Self::LicenseForbidden { .. } => 403,
            Self::NotImplemented(_) => 501,
            _ => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_request_400() {
        assert_eq!(EdgeForgeError::InvalidRequest("bad".into()).http_status(), 400);
    }

    #[test]
    fn test_invalid_preset_400() {
        assert_eq!(EdgeForgeError::InvalidPreset("bad".into()).http_status(), 400);
    }

    #[test]
    fn test_unauthorized_401() {
        assert_eq!(EdgeForgeError::Unauthorized("no key".into()).http_status(), 401);
    }

    #[test]
    fn test_mtls_failed_401() {
        assert_eq!(EdgeForgeError::MtlsHandshakeFailed("cert error".into()).http_status(), 401);
    }

    #[test]
    fn test_rate_limited_429() {
        assert_eq!(EdgeForgeError::RateLimited("too fast".into()).http_status(), 429);
    }

    #[test]
    fn test_model_not_found_404() {
        assert_eq!(EdgeForgeError::ModelNotFound("01J9".into()).http_status(), 404);
    }

    #[test]
    fn test_job_not_found_404() {
        assert_eq!(EdgeForgeError::JobNotFound("01J9".into()).http_status(), 404);
    }

    #[test]
    fn test_invalid_job_state_409() {
        assert_eq!(
            EdgeForgeError::InvalidJobState { job: "j1".into(), state: "running".into() }.http_status(),
            409
        );
    }

    #[test]
    fn test_license_forbidden_403() {
        assert_eq!(
            EdgeForgeError::LicenseForbidden { license: "GPL".into(), use_case: "commercial".into() }.http_status(),
            403
        );
    }

    #[test]
    fn test_not_implemented_501() {
        assert_eq!(EdgeForgeError::NotImplemented("stub").http_status(), 501);
    }

    #[test]
    fn test_engine_error_500() {
        assert_eq!(EdgeForgeError::Engine("crash".into()).http_status(), 500);
    }

    #[test]
    fn test_database_error_500() {
        assert_eq!(EdgeForgeError::Database("locked".into()).http_status(), 500);
    }

    #[test]
    fn test_config_error_500() {
        assert_eq!(EdgeForgeError::Config("bad file".into()).http_status(), 500);
    }

    #[test]
    fn test_mesh_error_500() {
        assert_eq!(EdgeForgeError::Mesh("disconnected".into()).http_status(), 500);
    }

    #[test]
    fn test_peer_unreachable_500() {
        assert_eq!(
            EdgeForgeError::PeerUnreachable { peer: "node1".into() }.http_status(),
            500
        );
    }

    #[test]
    fn test_sandbox_denied_500() {
        assert_eq!(EdgeForgeError::SandboxDenied("no access".into()).http_status(), 500);
    }

    #[test]
    fn test_io_error_500() {
        let err = EdgeForgeError::Io {
            path: std::path::PathBuf::from("/tmp/test"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        assert_eq!(err.http_status(), 500);
    }

    #[test]
    fn test_thermal_critical_500() {
        assert_eq!(EdgeForgeError::SsdThermalCritical { temp_celsius: 85.0 }.http_status(), 500);
    }

    #[test]
    fn test_model_load_failed_500() {
        assert_eq!(
            EdgeForgeError::ModelLoadFailed { model: "m1".into(), reason: "corrupt".into() }.http_status(),
            500
        );
    }

    #[test]
    fn test_manifest_signature_invalid_500() {
        assert_eq!(EdgeForgeError::ManifestSignatureInvalid("bad sig".into()).http_status(), 500);
    }

    #[test]
    fn test_integrity_mismatch_500() {
        assert_eq!(
            EdgeForgeError::IntegrityMismatch { expected: "abc".into(), actual: "def".into() }.http_status(),
            500
        );
    }

    #[test]
    fn test_insufficient_ssd_500() {
        assert_eq!(
            EdgeForgeError::InsufficientSsdSpace { needed: 100, have: 50 }.http_status(),
            500
        );
    }

    #[test]
    fn test_error_display() {
        let err = EdgeForgeError::ModelNotFound("01J9".into());
        assert!(err.to_string().contains("01J9"));
    }
}
