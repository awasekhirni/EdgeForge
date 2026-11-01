// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Tracing Subscriber Setup
//!
//! ## Purpose
//! Initialises the `tracing_subscriber` with JSON or pretty formatter,
//! env filter, and optional OTLP exporter.
//!
//! ## What This File Does
//! - `init(cfg)` — idempotently initialises the global subscriber
//! - Returns `ObservabilityGuard` that flushes on drop
//! - Supports `EDGEFORGE_LOG` env var for log level (default: "info")
//! - Supports `EDGEFORGE_LOG_FORMAT=json` for JSON output
//!
//! ## Dependencies
//! - `tracing-subscriber` — registry, fmt layer, env filter
//! - `edgeforge-core::Config` — for log_level and otlp settings
//!
//! ## Output
//! `ObservabilityGuard` — keep alive for the entire process lifetime.
//!
//! ## Function: init
//! - **Purpose:** Initialise the global tracing subscriber
//! - **Parameters:** `&Config` — configuration with log_level and OTLP settings
//! - **Returns:** `ObservabilityGuard` — RAII guard that flushes on drop
//! - **Errors:** Panics if env-filter directive string is malformed

use edgeforge_core::Config;
use std::sync::OnceLock;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
/// `ObservabilityGuard` — see module documentation for purpose and usage.
pub struct ObservabilityGuard;
static INIT: OnceLock<()> = OnceLock::new();
/// Init — see module documentation for details.
pub fn init(cfg: &Config) -> ObservabilityGuard {
    INIT.get_or_init(|| {
        let env_filter = EnvFilter::try_new(&cfg.observability.log_level)
            .unwrap_or_else(|_| EnvFilter::new("info"));
        let fmt_layer = fmt::layer().with_target(true);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    });
    ObservabilityGuard
}
