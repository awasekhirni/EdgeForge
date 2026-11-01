//! # Real-Time Performance Monitor
//!
//! ## Purpose
//! Continuously tracks the performance of all modules against the current
//! hardware. Takes periodic snapshots of system resources (CPU, RAM, SSD)
//! and correlates them with function-call performance data.
//!
//! ## What This File Does
//! - `PerfMonitor` — real-time performance monitor
//! - `PerfSnapshot` — a point-in-time snapshot of system + function performance
//! - `PerfMonitorConfig` — monitoring interval, thresholds
//! - `take_snapshot()` — capture a performance snapshot
//! - `check_regression()` — compare current performance against a baseline
//!
//! ## Dependencies
//! - `edgeforge-perf::hardware` — HardwareSpec
//! - `edgeforge-perf::function_perf` — FunctionPerfTracker
//! - `edgeforge-perf::comparative` — ComparativeRegistry
//! - `edgeforge-observability` — FunctionGuard
//! - `sysinfo` — system resource monitoring
//!
//! ## Output
//! `PerfSnapshot` entries with system metrics + function stats.
//!
//! ## Function Logging
//! `timestamp - edgeforge-perf - monitor.rs - function - STATUS`

use crate::comparative::ComparativeRegistry;
use crate::function_perf::{FunctionPerfTracker, FunctionStats};
use crate::hardware::HardwareSpec;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Configuration for the performance monitor.
#[derive(Clone, Debug)]
pub struct PerfMonitorConfig {
    /// Snapshot interval in seconds (default: 10).
    pub interval_secs: u64,
    /// Regression threshold: if current p99 > baseline_p99 * (1 + threshold),
    /// it's a regression (default: 0.20 = 20%).
    pub regression_threshold: f64,
}

impl Default for PerfMonitorConfig {
    fn default() -> Self {
        Self {
            interval_secs: 10,
            regression_threshold: 0.20,
        }
    }
}

/// A point-in-time performance snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfSnapshot {
    /// Timestamp (Unix ms).
    pub timestamp_ms: i64,
    /// CPU usage percentage (0.0–100.0).
    pub cpu_usage_percent: f32,
    /// RAM usage percentage (0.0–100.0).
    pub ram_usage_percent: f32,
    /// Available RAM in bytes.
    pub available_ram_bytes: u64,
    /// SSD usage percentage of the model storage drive (if detectable).
    pub ssd_usage_percent: Option<f32>,
    /// SSD temperature in Celsius (if detectable).
    pub ssd_temp_celsius: Option<f32>,
    /// Number of tracked functions.
    pub tracked_functions: usize,
    /// Total number of recorded calls.
    pub total_calls: u64,
    /// Top 5 slowest functions (by p99).
    pub slowest_functions: Vec<FunctionStats>,
    /// Hardware signature.
    pub hardware_signature: String,
}

/// Performance regression alert.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegressionAlert {
    /// The function that regressed.
    pub function_name: String,
    /// Baseline p99 (microseconds).
    pub baseline_p99_us: u64,
    /// Current p99 (microseconds).
    pub current_p99_us: u64,
    /// Percentage increase.
    pub percent_increase: f64,
    /// Severity: "minor" (<50%), "major" (<100%), "critical" (≥100%).
    pub severity: RegressionSeverity,
}

/// Severity of a performance regression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionSeverity {
    /// <50% increase.
    Minor,
    /// 50–100% increase.
    Major,
    /// ≥100% increase.
    Critical,
}

/// Real-time performance monitor.
///
/// ## Purpose
/// Tracks module performance against the current hardware. Takes periodic
/// snapshots, detects regressions, and logs alerts.
pub struct PerfMonitor {
    /// Monitor configuration.
    pub config: PerfMonitorConfig,
    /// The hardware specification (captured at init).
    pub hardware: HardwareSpec,
    /// Reference to the function performance tracker.
    pub tracker: std::sync::Arc<FunctionPerfTracker>,
    /// Reference to the comparative benchmark registry.
    pub registry: std::sync::Arc<ComparativeRegistry>,
    /// Baseline function stats (for regression detection).
    baseline: parking_lot::RwLock<Vec<FunctionStats>>,
}

impl PerfMonitor {
    /// Creates a new performance monitor.
    ///
    /// ## Parameters
    /// - `config`: Monitor configuration.
    /// - `tracker`: Function performance tracker.
    /// - `registry`: Comparative benchmark registry.
    ///
    /// ## Returns
    /// A new `PerfMonitor` instance with the current hardware profiled.
    ///
    /// ## Errors
    /// Returns an error if hardware profiling fails.
    pub fn new(
        config: PerfMonitorConfig,
        tracker: std::sync::Arc<FunctionPerfTracker>,
        registry: std::sync::Arc<ComparativeRegistry>,
    ) -> Result<Self, String> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "monitor.rs",
            "PerfMonitor::new",
        );
        let hardware = crate::hardware::profile_hardware()?;
        guard.success();
        Ok(Self {
            config,
            hardware,
            tracker,
            registry,
            baseline: parking_lot::RwLock::new(Vec::new()),
        })
    }

    /// Capture a performance snapshot.
    ///
    /// ## Purpose
    /// Takes a point-in-time snapshot of system resources (CPU, RAM, SSD)
    /// and function performance stats.
    ///
    /// ## Returns
    /// `PerfSnapshot` — the captured snapshot.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-perf - monitor.rs - take_snapshot - STATUS`
    pub fn take_snapshot(&self) -> PerfSnapshot {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "monitor.rs",
            "take_snapshot",
        );
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();

        let cpu_usage = sys.global_cpu_usage();
        let total_ram = sys.total_memory();
        let available_ram = sys.available_memory();
        let ram_usage = if total_ram > 0 {
            ((total_ram - available_ram) as f32 / total_ram as f32) * 100.0
        } else {
            0.0
        };

        let all_stats = self.tracker.get_all_stats();
        let total_calls: u64 = all_stats.iter().map(|s| s.call_count).sum();

        // Top 5 slowest by p99.
        let mut slowest = all_stats.clone();
        slowest.sort_by(|a, b| b.p99_duration_us.cmp(&a.p99_duration_us));
        slowest.truncate(5);

        PerfSnapshot {
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            cpu_usage_percent: cpu_usage,
            ram_usage_percent: ram_usage,
            available_ram_bytes: available_ram,
            ssd_usage_percent: None,
            ssd_temp_celsius: None,
            tracked_functions: all_stats.len(),
            total_calls,
            slowest_functions: slowest,
            hardware_signature: self.hardware.signature.clone(),
        }
    }

    /// Set the baseline for regression detection.
    ///
    /// ## Purpose
    /// Captures the current function stats as the baseline. Future calls
    /// to `check_regression()` will compare against this baseline.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-perf - monitor.rs - set_baseline - STATUS`
    pub fn set_baseline(&self) {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "monitor.rs",
            "set_baseline",
        );
        let stats = self.tracker.get_all_stats();
        *self.baseline.write() = stats;
        tracing::info!(
            functions = self.baseline.read().len(),
            "Performance baseline set"
        );
    }

    /// Check for performance regressions against the baseline.
    ///
    /// ## Purpose
    /// Compares the current function stats against the baseline. Returns
    /// alerts for any function whose p99 has regressed beyond the threshold.
    ///
    /// ## Returns
    /// `Vec<RegressionAlert>` — alerts for regressed functions.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-perf - monitor.rs - check_regression - STATUS`
    pub fn check_regression(&self) -> Vec<RegressionAlert> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "monitor.rs",
            "check_regression",
        );
        let baseline = self.baseline.read();
        if baseline.is_empty() {
            return Vec::new();
        }

        let current = self.tracker.get_all_stats();
        let mut alerts = Vec::new();

        for base in baseline.iter() {
            if let Some(curr) = current.iter().find(|c| c.function_name == base.function_name) {
                let threshold_p99 = (base.p99_duration_us as f64
                    * (1.0 + self.config.regression_threshold))
                    as u64;
                if curr.p99_duration_us > threshold_p99 {
                    let pct_increase = if base.p99_duration_us > 0 {
                        ((curr.p99_duration_us as f64 - base.p99_duration_us as f64)
                            / base.p99_duration_us as f64)
                            * 100.0
                    } else {
                        0.0
                    };
                    let severity = if pct_increase >= 100.0 {
                        RegressionSeverity::Critical
                    } else if pct_increase >= 50.0 {
                        RegressionSeverity::Major
                    } else {
                        RegressionSeverity::Minor
                    };
                    alerts.push(RegressionAlert {
                        function_name: base.function_name.clone(),
                        baseline_p99_us: base.p99_duration_us,
                        current_p99_us: curr.p99_duration_us,
                        percent_increase: pct_increase,
                        severity,
                    });
                }
            }
        }

        if !alerts.is_empty() {
            tracing::warn!(
                regressions = alerts.len(),
                "Performance regressions detected"
            );
        }

        alerts
    }

    /// Returns the hardware specification.
    pub fn hardware(&self) -> &HardwareSpec {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "monitor.rs",
            "hardware",
        );
        &self.hardware
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take_snapshot() {
        let tracker = std::sync::Arc::new(FunctionPerfTracker::with_signature("test"));
        let registry = std::sync::Arc::new(ComparativeRegistry::new());
        let monitor = PerfMonitor::new(
            PerfMonitorConfig::default(),
            tracker.clone(),
            registry,
        )
        .unwrap();

        // Record some calls.
        tracker.record_call("c", "f.rs", "fn1", crate::function_perf::FunctionKpi::success(100), None);
        tracker.record_call("c", "f.rs", "fn2", crate::function_perf::FunctionKpi::success(200), None);

        let snapshot = monitor.take_snapshot();
        assert!(snapshot.tracked_functions >= 2);
        assert!(snapshot.total_calls >= 2);
        assert!(!snapshot.hardware_signature.is_empty());
    }

    #[test]
    fn test_regression_detection() {
        let tracker = std::sync::Arc::new(FunctionPerfTracker::with_signature("test"));
        let registry = std::sync::Arc::new(ComparativeRegistry::new());
        let monitor = PerfMonitor::new(
            PerfMonitorConfig {
                regression_threshold: 0.10, // 10% threshold
                ..Default::default()
            },
            tracker.clone(),
            registry,
        )
        .unwrap();

        // Baseline: 100µs.
        tracker.record_call("c", "f.rs", "fn1", crate::function_perf::FunctionKpi::success(100), None);
        monitor.set_baseline();

        // Clear and record slower calls.
        tracker.clear();
        tracker.record_call("c", "f.rs", "fn1", crate::function_perf::FunctionKpi::success(200), None);

        let alerts = monitor.check_regression();
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].function_name, "fn1");
        assert!(alerts[0].percent_increase > 50.0);
    }

    #[test]
    fn test_no_regression_when_within_threshold() {
        let tracker = std::sync::Arc::new(FunctionPerfTracker::with_signature("test"));
        let registry = std::sync::Arc::new(ComparativeRegistry::new());
        let monitor = PerfMonitor::new(
            PerfMonitorConfig {
                regression_threshold: 0.50, // 50% threshold
                ..Default::default()
            },
            tracker.clone(),
            registry,
        )
        .unwrap();

        tracker.record_call("c", "f.rs", "fn1", crate::function_perf::FunctionKpi::success(100), None);
        monitor.set_baseline();
        tracker.clear();
        tracker.record_call("c", "f.rs", "fn1", crate::function_perf::FunctionKpi::success(110), None); // 10% increase

        let alerts = monitor.check_regression();
        assert!(alerts.is_empty()); // 10% < 50% threshold
    }

    #[test]
    fn test_no_regression_without_baseline() {
        let tracker = std::sync::Arc::new(FunctionPerfTracker::with_signature("test"));
        let registry = std::sync::Arc::new(ComparativeRegistry::new());
        let monitor = PerfMonitor::new(
            PerfMonitorConfig::default(),
            tracker,
            registry,
        )
        .unwrap();
        let alerts = monitor.check_regression();
        assert!(alerts.is_empty()); // No baseline set
    }

    #[test]
    fn test_snapshot_has_slowest_functions() {
        let tracker = std::sync::Arc::new(FunctionPerfTracker::with_signature("test"));
        let registry = std::sync::Arc::new(ComparativeRegistry::new());
        let monitor = PerfMonitor::new(
            PerfMonitorConfig::default(),
            tracker.clone(),
            registry,
        )
        .unwrap();

        tracker.record_call("c", "f.rs", "fast_fn", crate::function_perf::FunctionKpi::success(10), None);
        tracker.record_call("c", "f.rs", "slow_fn", crate::function_perf::FunctionKpi::success(1000), None);

        let snapshot = monitor.take_snapshot();
        assert!(!snapshot.slowest_functions.is_empty());
        // The slowest function should be first.
        assert_eq!(snapshot.slowest_functions[0].function_name, "slow_fn");
    }
}
