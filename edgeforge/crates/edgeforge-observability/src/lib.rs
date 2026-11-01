// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-observability — Tracing + Opt-In OTLP + Function Logger (ADR-012)
//!
//! ## Purpose
//! Provides structured logging via `tracing`, optional OpenTelemetry OTLP
//! export behind a feature flag, and a comprehensive function-call logger
//! that records every function invocation in the format:
//! `timestamp - crate - file - function - status`
//!
//! ## Modules
//! - `subscriber` — tracing subscriber setup with env filter
//! - `function_logger` — world-class function-call logging with CallLog, FunctionGuard
//!
//! ## Dependencies
//! - `edgeforge-core` — for Config
//! - `tracing` — structured logging
//! - `tracing-subscriber` — subscriber setup
//! - `chrono` — ISO 8601 timestamps
//! - `parking_lot` — thread-safe log buffer
//! - `serde` / `serde_json` — log serialization
//!
//! ## Output
//! `ObservabilityGuard` that flushes the OTLP exporter on drop.
//! `FunctionLogger` that accumulates function-call logs for audit.
//!
//! ## Safety (R-14)
//! Span attributes must never include prompt content. The
//! `FORBIDDEN_ATTRIBUTE_SUBSTRINGS` list is enforced.

pub mod function_logger;
pub mod subscriber;

pub use subscriber::init;
pub use subscriber::ObservabilityGuard;

// Re-export the log_call! macro at crate root for convenience.
pub use function_logger::{CallLog, CallStatus, FunctionGuard, FunctionLogger};
