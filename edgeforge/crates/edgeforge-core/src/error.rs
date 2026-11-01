// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
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
    #[error("manifest signature invalid: {0}")]
    ManifestSignatureInvalid(String),
    #[error("integrity mismatch: expected {expected}, got {actual}")]
    IntegrityMismatch { expected: String, actual: String },
    #[error("license forbidden: {license} for {use_case}")]
    LicenseForbidden { license: String, use_case: String },
    #[error("sandbox denied: {0}")]
    SandboxDenied(String),
    #[error("mesh error: {0}")]
    Mesh(String),
    #[error("peer unreachable: {peer}")]
    PeerUnreachable { peer: String },
    #[error("mTLS failed: {0}")]
    MtlsHandshakeFailed(String),
    #[error("I/O error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("insufficient SSD: need {needed}, have {have}")]
    InsufficientSsdSpace { needed: u64, have: u64 },
    #[error("SSD thermal critical: {temp_celsius}°C")]
    SsdThermalCritical { temp_celsius: f32 },
    #[error("model load failed: {model}: {reason}")]
    ModelLoadFailed { model: String, reason: String },
    #[error("engine error: {0}")]
    Engine(String),
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("invalid preset: {0}")]
    InvalidPreset(String),
    #[error("invalid job state: {job} is {state}")]
    InvalidJobState { job: String, state: String },
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
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
