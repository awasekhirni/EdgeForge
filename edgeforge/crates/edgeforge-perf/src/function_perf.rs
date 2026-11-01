// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Per-Function Performance Tracking with KPI Metrics
//!
//! ## Purpose
//! Tracks the performance of every function call — including duration,
//! memory usage, and KPI metrics — against the current hardware configuration.
//! Each function's performance is logged with the hardware signature so
//! results can be compared across platforms.
//!
//! ## What This File Does
//! - `FunctionKpi` — KPI metrics for a single function call
//! - `FunctionPerfLog` — a complete performance log entry
//! - `FunctionPerfTracker` — thread-safe tracker that accumulates per-function stats
//! - `record_call()` — record a function call with KPI metrics
//! - `get_stats()` — get aggregated stats for a function
//! - `dump_json()` — export all performance data as JSON
//! - `get_all_function_names()` — list all tracked functions
//!
//! ## Dependencies
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `edgeforge-perf::hardware` — HardwareSpec for hardware attribution
//! - `parking_lot` — thread-safe state
//! - `serde` — serialization
//!
//! ## Output
//! `FunctionPerfLog` entries with: function name, crate, file, duration,
//! memory delta, hardware signature, timestamp.
//!
//! ## Function Logging
//! `timestamp - edgeforge-perf - function_perf.rs - function - STATUS`

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// KPI metrics for a single function call.
///
/// ## Purpose
/// Captures the key performance indicators for a function invocation,
/// including timing, memory, and I/O metrics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionKpi {
    /// Wall-clock duration in microseconds.
    pub duration_us: u64,
    /// Memory allocated during the call (bytes, if measurable).
    pub memory_delta_bytes: Option<i64>,
    /// Number of I/O operations (if applicable).
    pub io_ops: Option<u32>,
    /// Bytes read from SSD (if applicable).
    pub ssd_bytes_read: Option<u64>,
    /// Bytes written to SSD (if applicable).
    pub ssd_bytes_written: Option<u64>,
    /// CPU usage percentage during the call (0.0–100.0, if measurable).
    pub cpu_usage_percent: Option<f32>,
    /// Whether the call was successful.
    pub success: bool,
    /// Error message (if failed).
    pub error: Option<String>,
}

impl FunctionKpi {
    /// Creates a KPI for a successful call with the given duration.
    pub fn success(duration_us: u64) -> Self {
        Self {
            duration_us,
            memory_delta_bytes: None,
            io_ops: None,
            ssd_bytes_read: None,
            ssd_bytes_written: None,
            cpu_usage_percent: None,
            success: true,
            error: None,
        }
    }

    /// Creates a KPI for a failed call with the given duration and error.
    pub fn error(duration_us: u64, error: impl Into<String>) -> Self {
        Self {
            duration_us,
            memory_delta_bytes: None,
            io_ops: None,
            ssd_bytes_read: None,
            ssd_bytes_written: None,
            cpu_usage_percent: None,
            success: false,
            error: Some(error.into()),
        }
    }
}

/// A complete performance log entry for a function call.
///
/// ## Purpose
/// Records the full context of a function call: what was called, when,
/// how long it took, on what hardware, and with what KPI metrics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionPerfLog {
    /// Timestamp (ISO 8601 UTC).
    pub timestamp: String,
    /// Crate name.
    pub crate_name: String,
    /// Source file name.
    pub file_name: String,
    /// Function name.
    pub function_name: String,
    /// KPI metrics for this call.
    pub kpi: FunctionKpi,
    /// Hardware signature (for comparative benchmarking).
    pub hardware_signature: String,
    /// Model ID (if the call was related to a specific model).
    pub model_id: Option<String>,
}

/// Aggregated statistics for a function.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionStats {
    /// Function name (crate::file::function).
    pub function_name: String,
    /// Number of calls recorded.
    pub call_count: u64,
    /// Mean duration in microseconds.
    pub mean_duration_us: f64,
    /// Min duration in microseconds.
    pub min_duration_us: u64,
    /// Max duration in microseconds.
    pub max_duration_us: u64,
    /// p50 (median) duration in microseconds.
    pub p50_duration_us: u64,
    /// p99 duration in microseconds.
    pub p99_duration_us: u64,
    /// Success rate (0.0–1.0).
    pub success_rate: f32,
    /// Hardware signature.
    pub hardware_signature: String,
}

/// Thread-safe per-function performance tracker.
///
/// ## Purpose
/// Accumulates performance data for every function call. Each call is
/// recorded with the hardware signature so results can be compared
/// across platforms.
pub struct FunctionPerfTracker {
    /// All recorded performance logs.
    logs: Mutex<Vec<FunctionPerfLog>>,
    /// The hardware signature for the current platform.
    hardware_signature: String,
}

impl FunctionPerfTracker {
    /// Creates a new performance tracker with the current hardware signature.
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "function_perf.rs",
            "FunctionPerfTracker::new",
        );
        let hw_sig = crate::hardware::profile_hardware()
            .map(|spec| spec.signature)
            .unwrap_or_else(|_| "unknown".to_string());
        Self {
            logs: Mutex::new(Vec::new()),
            hardware_signature: hw_sig,
        }
    }

    /// Creates a tracker with a custom hardware signature (for testing).
    pub fn with_signature(hardware_signature: impl Into<String>) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "function_perf.rs",
            "with_signature",
        );
        Self {
            logs: Mutex::new(Vec::new()),
            hardware_signature: hardware_signature.into(),
        }
    }

    /// Record a function call with KPI metrics.
    ///
    /// ## Parameters
    /// - `crate_name`: Name of the crate.
    /// - `file_name`: Source file name.
    /// - `function_name`: Function name.
    /// - `kpi`: KPI metrics for this call.
    /// - `model_id`: Optional model ID (if the call was model-specific).
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-perf - function_perf.rs - record_call - STATUS`
    pub fn record_call(
        &self,
        crate_name: &str,
        file_name: &str,
        function_name: &str,
        kpi: FunctionKpi,
        model_id: Option<&str>,
    ) {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-perf", "function_perf.rs", "record_call");
        let entry = FunctionPerfLog {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            crate_name: crate_name.to_string(),
            file_name: file_name.to_string(),
            function_name: function_name.to_string(),
            kpi,
            hardware_signature: self.hardware_signature.clone(),
            model_id: model_id.map(|s| s.to_string()),
        };

        tracing::debug!(
            crate = %entry.crate_name,
            file = %entry.file_name,
            function = %entry.function_name,
            duration_us = entry.kpi.duration_us,
            success = entry.kpi.success,
            hw_sig = %entry.hardware_signature,
            "PERF_LOG"
        );

        let mut logs = self.logs.lock();
        // Bound to 50,000 entries.
        if logs.len() >= 50_000 {
            logs.remove(0);
        }
        logs.push(entry);
    }

    /// Get aggregated statistics for a specific function.
    ///
    /// ## Parameters
    /// - `function_name`: The function name to query.
    ///
    /// ## Returns
    /// `Option<FunctionStats>` — aggregated stats, or None if no calls recorded.
    pub fn get_stats(&self, function_name: &str) -> Option<FunctionStats> {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "function_perf.rs", "get_stats");
        let logs = self.logs.lock();
        let matching: Vec<&FunctionPerfLog> = logs
            .iter()
            .filter(|l| l.function_name == function_name)
            .collect();
        if matching.is_empty() {
            return None;
        }

        let mut durations: Vec<u64> = matching.iter().map(|l| l.kpi.duration_us).collect();
        durations.sort_unstable();

        let call_count = matching.len() as u64;
        let sum: u64 = durations.iter().sum();
        let mean = sum as f64 / call_count as f64;
        let min = *durations.first().unwrap();
        let max = *durations.last().unwrap();
        let p50 = durations[(call_count as usize * 50 / 100).min(durations.len() - 1)];
        let p99 = durations[(call_count as usize * 99 / 100).min(durations.len() - 1)];
        let success_count = matching.iter().filter(|l| l.kpi.success).count() as u32;
        let success_rate = success_count as f32 / call_count as f32;

        Some(FunctionStats {
            function_name: function_name.to_string(),
            call_count,
            mean_duration_us: mean,
            min_duration_us: min,
            max_duration_us: max,
            p50_duration_us: p50,
            p99_duration_us: p99,
            success_rate,
            hardware_signature: self.hardware_signature.clone(),
        })
    }

    /// Get all tracked function names.
    pub fn get_all_function_names(&self) -> Vec<String> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "function_perf.rs",
            "get_all_function_names",
        );
        let logs = self.logs.lock();
        let mut names: Vec<String> = logs
            .iter()
            .map(|l| l.function_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        names.sort();
        names
    }

    /// Get stats for all tracked functions.
    pub fn get_all_stats(&self) -> Vec<FunctionStats> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "function_perf.rs",
            "get_all_stats",
        );
        self.get_all_function_names()
            .iter()
            .filter_map(|name| self.get_stats(name))
            .collect()
    }

    /// Dump all performance logs as JSON.
    pub fn dump_json(&self) -> String {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "function_perf.rs", "dump_json");
        let logs = self.logs.lock();
        serde_json::to_string_pretty(logs.as_slice()).unwrap_or_else(|_| "[]".to_string())
    }

    /// Dump all stats as JSON.
    pub fn dump_stats_json(&self) -> String {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "function_perf.rs",
            "dump_stats_json",
        );
        let stats = self.get_all_stats();
        serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "[]".to_string())
    }

    /// Returns the number of recorded calls.
    pub fn len(&self) -> usize {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "function_perf.rs", "len");
        self.logs.lock().len()
    }

    /// Returns true if no calls have been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the hardware signature.
    pub fn hardware_signature(&self) -> &str {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "function_perf.rs",
            "hardware_signature",
        );
        &self.hardware_signature
    }

    /// Clear all recorded logs.
    pub fn clear(&self) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "function_perf.rs", "clear");
        self.logs.lock().clear();
    }
}

impl Default for FunctionPerfTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for tracking function performance.
///
/// ## Purpose
/// Automatically records the function's KPI metrics (duration, success/error)
/// when the guard is dropped. Similar to `FunctionGuard` from edgeforge-observability
/// but also records KPI metrics to the `FunctionPerfTracker`.
///
/// ## Usage
/// ```ignore
/// use edgeforge_perf::function_perf::{PerfGuard, PERF_TRACKER};
///
/// pub fn my_function() -> Result<(), Error> {
///     let _guard = PerfGuard::new("edgeforge-runtime", "engine.rs", "generate");
///     // ... function body ...
///     Ok(())
/// }
/// ```
pub struct PerfGuard {
    crate_name: &'static str,
    file_name: &'static str,
    function_name: &'static str,
    start: Instant,
    success: bool,
    error_msg: Option<String>,
}

impl PerfGuard {
    /// Creates a new performance guard and starts the timer.
    pub fn new(
        crate_name: &'static str,
        file_name: &'static str,
        function_name: &'static str,
    ) -> Self {
        Self {
            crate_name,
            file_name,
            function_name,
            start: Instant::now(),
            success: true,
            error_msg: None,
        }
    }

    /// Mark the function as failed.
    pub fn error(&mut self, msg: impl Into<String>) {
        self.success = false;
        self.error_msg = Some(msg.into());
    }
}

impl Drop for PerfGuard {
    fn drop(&mut self) {
        let duration_us = self.start.elapsed().as_micros() as u64;
        let kpi = if self.success {
            FunctionKpi::success(duration_us)
        } else {
            FunctionKpi::error(duration_us, self.error_msg.take().unwrap_or_default())
        };
        crate::PERF_TRACKER.record_call(
            self.crate_name,
            self.file_name,
            self.function_name,
            kpi,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_get_stats() {
        let tracker = FunctionPerfTracker::with_signature("test-platform");
        tracker.record_call(
            "test-crate",
            "file.rs",
            "fn1",
            FunctionKpi::success(100),
            None,
        );
        tracker.record_call(
            "test-crate",
            "file.rs",
            "fn1",
            FunctionKpi::success(200),
            None,
        );
        tracker.record_call(
            "test-crate",
            "file.rs",
            "fn1",
            FunctionKpi::error(150, "err"),
            None,
        );

        let stats = tracker.get_stats("fn1").unwrap();
        assert_eq!(stats.call_count, 3);
        assert_eq!(stats.min_duration_us, 100);
        assert_eq!(stats.max_duration_us, 200);
        assert!((stats.success_rate - 0.667).abs() < 0.1);
    }

    #[test]
    fn test_get_all_function_names() {
        let tracker = FunctionPerfTracker::with_signature("test");
        tracker.record_call("c", "f.rs", "fn_a", FunctionKpi::success(10), None);
        tracker.record_call("c", "f.rs", "fn_b", FunctionKpi::success(20), None);
        let names = tracker.get_all_function_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"fn_a".to_string()));
        assert!(names.contains(&"fn_b".to_string()));
    }

    #[test]
    fn test_dump_json() {
        let tracker = FunctionPerfTracker::with_signature("test");
        tracker.record_call("c", "f.rs", "fn", FunctionKpi::success(42), None);
        let json = tracker.dump_json();
        assert!(json.contains("fn"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_dump_stats_json() {
        let tracker = FunctionPerfTracker::with_signature("test");
        tracker.record_call("c", "f.rs", "fn", FunctionKpi::success(42), None);
        let json = tracker.dump_stats_json();
        assert!(json.contains("call_count"));
        assert!(json.contains("mean_duration_us"));
    }

    #[test]
    fn test_perf_guard_success() {
        let tracker = FunctionPerfTracker::with_signature("test");
        // We can't use the global PERF_TRACKER in tests, so test the guard logic manually.
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(1));
        let duration_us = start.elapsed().as_micros() as u64;
        tracker.record_call(
            "c",
            "f.rs",
            "test_guard",
            FunctionKpi::success(duration_us),
            None,
        );
        let stats = tracker.get_stats("test_guard").unwrap();
        assert!(stats.mean_duration_us > 0.0);
    }

    #[test]
    fn test_perf_guard_error() {
        let tracker = FunctionPerfTracker::with_signature("test");
        tracker.record_call(
            "c",
            "f.rs",
            "test_err",
            FunctionKpi::error(500, "fail"),
            None,
        );
        let stats = tracker.get_stats("test_err").unwrap();
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_clear() {
        let tracker = FunctionPerfTracker::with_signature("test");
        tracker.record_call("c", "f.rs", "fn", FunctionKpi::success(10), None);
        assert!(!tracker.is_empty());
        tracker.clear();
        assert!(tracker.is_empty());
    }

    #[test]
    fn test_hardware_signature_attribution() {
        let tracker = FunctionPerfTracker::with_signature("ryzen9-64gb-linux");
        tracker.record_call("c", "f.rs", "fn", FunctionKpi::success(10), None);
        let stats = tracker.get_stats("fn").unwrap();
        assert_eq!(stats.hardware_signature, "ryzen9-64gb-linux");
    }

    #[test]
    fn test_model_id_attribution() {
        let tracker = FunctionPerfTracker::with_signature("test");
        tracker.record_call(
            "c",
            "f.rs",
            "fn",
            FunctionKpi::success(10),
            Some("model-123"),
        );
        let logs = tracker.dump_json();
        assert!(logs.contains("model-123"));
    }
}
