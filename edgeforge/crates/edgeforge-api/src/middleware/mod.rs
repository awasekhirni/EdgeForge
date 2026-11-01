// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Axum Middleware
//!
//! ## Purpose
//! Centralized error handling, rate limiting, authentication, verification
//! header injection, and structured logging.
//!
//! ## Modules
//! - `auth` — API key authentication (Authorization: Bearer header)
//! - `error` — centralized error → JSON response mapping
//! - `logging` — structured request/response logging
//! - `rate_limit` — per-IP token-bucket rate limiting
//! - `verification` — X-EdgeForge-Verification header injection (ADR-018)

pub mod auth;
pub mod error;
pub mod logging;
pub mod rate_limit;
pub mod verification;

pub use error::ApiError;
