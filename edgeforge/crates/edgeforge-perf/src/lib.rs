// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-perf — Performance Monitoring & Comparative Benchmarking
//!
//! ## Purpose
//! Comprehensive performance monitoring module that:
//! - Profiles the current hardware platform in detail (CPU, RAM, SSD, GPU)
//! - Measures and logs the performance of every function against the current hardware
//! - Maintains a comparative benchmark registry for known hardware platforms
//! - Logs KPI performance metrics per function call
//! - Tracks performance regressions across releases
//!
//! ## What This File Does
//! This is the crate root. It declares all public modules and re-exports
//! the most commonly used types.
//!
//! ## Modules
//! - `hardware` — hardware specification profiler (CPU, RAM, SSD, GPU)
//! - `function_perf` — per-function performance tracking with KPI metrics
//! - `comparative` — comparative benchmark registry for known hardware platforms
//! - `monitor` — real-time performance monitor for tracking module performance
//!
//! ## Dependencies
//! - `edgeforge-core` — types, errors
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `sysinfo` — system information gathering
//! - `parking_lot` — thread-safe state
//! - `chrono` — timestamps
//!
//! ## Output
//! `HardwareSpec` — current hardware specification
//! `FunctionPerfLog` — per-function performance log entry
//! `ComparativeBenchmark` — benchmark result for a known platform
//!
//! ## Function Logging
//! `timestamp - edgeforge-perf - lib.rs - function - STATUS`

pub mod comparative;
pub mod function_perf;
pub mod hardware;
pub mod monitor;

pub use comparative::{ComparativeBenchmark, ComparativeRegistry, PlatformId};
pub use function_perf::{FunctionKpi, FunctionPerfLog, FunctionPerfTracker};
pub use hardware::{CpuSpec, GpuSpec, HardwareSpec, RamSpec, SsdSpec};
pub use monitor::{PerfMonitor, PerfMonitorConfig, PerfSnapshot};

/// Global performance tracker for the edgeforge-perf crate.
pub static PERF_TRACKER: std::sync::LazyLock<FunctionPerfTracker> =
    std::sync::LazyLock::new(FunctionPerfTracker::new);
