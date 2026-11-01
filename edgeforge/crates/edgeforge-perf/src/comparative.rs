// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Comparative Benchmark Registry
//!
//! ## Purpose
//! Maintains a registry of known hardware platforms and their benchmark
//! results. Allows comparative analysis: "How does function X perform
//! on platform A vs platform B?"
//!
//! ## What This File Does
//! - `PlatformId` — identifies a known hardware platform
//! - `ComparativeBenchmark` — benchmark result for a function on a platform
//! - `ComparativeRegistry` — thread-safe registry of all benchmarks
//! - `register_benchmark()` — add a benchmark result
//! - `compare()` — compare a function's performance across platforms
//! - `get_platform_benchmarks()` — get all benchmarks for a platform
//! - `KNOWN_PLATFORMS` — predefined reference platforms
//!
//! ## Dependencies
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `parking_lot` — thread-safe registry
//! - `serde` — serialization
//!
//! ## Output
//! `ComparativeBenchmark` entries and comparison reports.
//!
//! ## Function Logging
//! `timestamp - edgeforge-perf - comparative.rs - function - STATUS`

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Identifies a known hardware platform for comparative benchmarking.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlatformId {
    /// Hardware signature (e.g., "ryzen9-7950x_64gb_nvme_linux").
    pub signature: String,
    /// Human-readable platform name.
    pub name: String,
}

/// A benchmark result for a specific function on a specific platform.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparativeBenchmark {
    /// The platform this benchmark was recorded on.
    pub platform: PlatformId,
    /// The function name (crate::file::function).
    pub function_name: String,
    /// Mean duration in microseconds.
    pub mean_duration_us: f64,
    /// p99 duration in microseconds.
    pub p99_duration_us: u64,
    /// Number of calls in the sample.
    pub call_count: u64,
    /// Success rate (0.0–1.0).
    pub success_rate: f32,
    /// When the benchmark was recorded (Unix ms).
    pub recorded_at_ms: i64,
    /// Model ID (if model-specific).
    pub model_id: Option<String>,
}

/// A comparison of a function's performance across two platforms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceComparison {
    /// The function name being compared.
    pub function_name: String,
    /// Platform A (baseline).
    pub platform_a: PlatformId,
    /// Platform B (comparison).
    pub platform_b: PlatformId,
    /// Mean duration on platform A (microseconds).
    pub mean_a_us: f64,
    /// Mean duration on platform B (microseconds).
    pub mean_b_us: f64,
    /// Speedup ratio (mean_a / mean_b). >1.0 = B is faster.
    pub speedup_ratio: f64,
    /// Percentage difference ((mean_b - mean_a) / mean_a * 100).
    pub percent_difference: f64,
    /// Verdict: "faster", "slower", or "same" (within 5%).
    pub verdict: ComparisonVerdict,
}

/// The verdict of a performance comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonVerdict {
    /// Platform B is faster (>5% improvement).
    Faster,
    /// Platform B is slower (>5% regression).
    Slower,
    /// Performance is within ±5% (same).
    Same,
}

/// Thread-safe registry of comparative benchmarks.
///
/// ## Purpose
/// Stores benchmark results for known hardware platforms. Allows querying
/// "How does function X perform on platform A vs platform B?"
pub struct ComparativeRegistry {
    /// Map: (platform_signature, function_name) → benchmark.
    benchmarks: RwLock<HashMap<(String, String), ComparativeBenchmark>>,
}

impl ComparativeRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "comparative.rs",
            "ComparativeRegistry::new",
        );
        Self {
            benchmarks: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a registry pre-populated with known reference platforms.
    pub fn with_known_platforms() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "comparative.rs",
            "with_known_platforms",
        );
        let registry = Self::new();
        for platform in known_platforms() {
            // Register placeholder benchmarks for known platforms.
            for (function, mean_us) in REFERENCE_FUNCTIONS {
                let benchmark = ComparativeBenchmark {
                    platform: platform.clone(),
                    function_name: function.to_string(),
                    mean_duration_us: mean_us.0,
                    p99_duration_us: mean_us.1,
                    call_count: 100,
                    success_rate: 1.0,
                    recorded_at_ms: 0,
                    model_id: None,
                };
                registry.register_benchmark(benchmark);
            }
        }
        registry
    }

    /// Register a benchmark result.
    ///
    /// ## Parameters
    /// - `benchmark`: The benchmark to register.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-perf - comparative.rs - register_benchmark - STATUS`
    pub fn register_benchmark(&self, benchmark: ComparativeBenchmark) {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "comparative.rs",
            "register_benchmark",
        );
        let key = (
            benchmark.platform.signature.clone(),
            benchmark.function_name.clone(),
        );
        self.benchmarks.write().insert(key, benchmark);
    }

    /// Compare a function's performance across two platforms.
    ///
    /// ## Parameters
    /// - `function_name`: The function to compare.
    /// - `platform_a`: The baseline platform.
    /// - `platform_b`: The comparison platform.
    ///
    /// ## Returns
    /// `Option<PerformanceComparison>` — None if either platform lacks data.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-perf - comparative.rs - compare - STATUS`
    pub fn compare(
        &self,
        function_name: &str,
        platform_a: &PlatformId,
        platform_b: &PlatformId,
    ) -> Option<PerformanceComparison> {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "comparative.rs", "compare");
        let benchmarks = self.benchmarks.read();
        let bench_a = benchmarks.get(&(platform_a.signature.clone(), function_name.to_string()))?;
        let bench_b = benchmarks.get(&(platform_b.signature.clone(), function_name.to_string()))?;

        let mean_a = bench_a.mean_duration_us;
        let mean_b = bench_b.mean_duration_us;
        let speedup_ratio = if mean_b > 0.0 { mean_a / mean_b } else { 0.0 };
        let percent_diff = if mean_a > 0.0 {
            ((mean_b - mean_a) / mean_a) * 100.0
        } else {
            0.0
        };
        let verdict = if percent_diff < -5.0 {
            ComparisonVerdict::Faster
        } else if percent_diff > 5.0 {
            ComparisonVerdict::Slower
        } else {
            ComparisonVerdict::Same
        };

        Some(PerformanceComparison {
            function_name: function_name.to_string(),
            platform_a: platform_a.clone(),
            platform_b: platform_b.clone(),
            mean_a_us: mean_a,
            mean_b_us: mean_b,
            speedup_ratio,
            percent_difference: percent_diff,
            verdict,
        })
    }

    /// Get all benchmarks for a specific platform.
    ///
    /// ## Parameters
    /// - `platform`: The platform to query.
    ///
    /// ## Returns
    /// `Vec<ComparativeBenchmark>` — all benchmarks for the platform.
    pub fn get_platform_benchmarks(&self, platform: &PlatformId) -> Vec<ComparativeBenchmark> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-perf",
            "comparative.rs",
            "get_platform_benchmarks",
        );
        self.benchmarks
            .read()
            .values()
            .filter(|b| b.platform.signature == platform.signature)
            .cloned()
            .collect()
    }

    /// Get all registered platform IDs.
    pub fn get_platforms(&self) -> Vec<PlatformId> {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-perf", "comparative.rs", "get_platforms");
        self.benchmarks
            .read()
            .values()
            .map(|b| b.platform.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Dump all benchmarks as JSON.
    pub fn dump_json(&self) -> String {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-perf", "comparative.rs", "dump_json");
        let benchmarks: Vec<ComparativeBenchmark> =
            self.benchmarks.read().values().cloned().collect();
        serde_json::to_string_pretty(&benchmarks).unwrap_or_else(|_| "[]".to_string())
    }
}

impl Default for ComparativeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Known reference hardware platforms for comparative benchmarking.
pub fn known_platforms() -> Vec<PlatformId> {
    vec![
        PlatformId {
            signature: "apple-m2-max_64gb_macos".to_string(),
            name: "Apple M2 Max (64GB Unified)".to_string(),
        },
        PlatformId {
            signature: "ryzen9-7950x_64gb_linux".to_string(),
            name: "AMD Ryzen 9 7950X (64GB DDR5)".to_string(),
        },
        PlatformId {
            signature: "intel-i9-13900k_32gb_linux".to_string(),
            name: "Intel Core i9-13900K (32GB DDR5)".to_string(),
        },
        PlatformId {
            signature: "ryzen9-5900x_32gb_linux".to_string(),
            name: "AMD Ryzen 9 5900X (32GB DDR4)".to_string(),
        },
        PlatformId {
            signature: "apple-m1-pro_16gb_macos".to_string(),
            name: "Apple M1 Pro (16GB Unified)".to_string(),
        },
    ]
}

/// Reference function benchmarks for known platforms.
/// (function_name, (mean_us, p99_us))
static REFERENCE_FUNCTIONS: &[(&str, (f64, u64))] = &[
    ("load", (5000.0, 15000)),
    ("generate", (2000.0, 8000)),
    ("page_in", (1500.0, 5000)),
    ("read_at", (500.0, 2000)),
    ("best_peer_for_expert", (8.0, 15)),
    ("predict", (100.0, 300)),
    ("tokenize", (300.0, 1000)),
    ("decode", (50000.0, 150000)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let registry = ComparativeRegistry::new();
        let platform = PlatformId {
            signature: "test-platform".to_string(),
            name: "Test Platform".to_string(),
        };
        let benchmark = ComparativeBenchmark {
            platform: platform.clone(),
            function_name: "test_fn".to_string(),
            mean_duration_us: 1000.0,
            p99_duration_us: 5000,
            call_count: 100,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        };
        registry.register_benchmark(benchmark);
        let results = registry.get_platform_benchmarks(&platform);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].function_name, "test_fn");
    }

    #[test]
    fn test_compare_faster() {
        let registry = ComparativeRegistry::new();
        let platform_a = PlatformId {
            signature: "slow".to_string(),
            name: "Slow".to_string(),
        };
        let platform_b = PlatformId {
            signature: "fast".to_string(),
            name: "Fast".to_string(),
        };
        registry.register_benchmark(ComparativeBenchmark {
            platform: platform_a.clone(),
            function_name: "fn".to_string(),
            mean_duration_us: 1000.0,
            p99_duration_us: 2000,
            call_count: 10,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        registry.register_benchmark(ComparativeBenchmark {
            platform: platform_b.clone(),
            function_name: "fn".to_string(),
            mean_duration_us: 500.0,
            p99_duration_us: 1000,
            call_count: 10,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        let comparison = registry.compare("fn", &platform_a, &platform_b).unwrap();
        assert_eq!(comparison.verdict, ComparisonVerdict::Faster);
        assert!(comparison.speedup_ratio > 1.0);
        assert!(comparison.percent_difference < -5.0);
    }

    #[test]
    fn test_compare_slower() {
        let registry = ComparativeRegistry::new();
        let platform_a = PlatformId {
            signature: "fast".to_string(),
            name: "Fast".to_string(),
        };
        let platform_b = PlatformId {
            signature: "slow".to_string(),
            name: "Slow".to_string(),
        };
        registry.register_benchmark(ComparativeBenchmark {
            platform: platform_a.clone(),
            function_name: "fn".to_string(),
            mean_duration_us: 500.0,
            p99_duration_us: 1000,
            call_count: 10,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        registry.register_benchmark(ComparativeBenchmark {
            platform: platform_b.clone(),
            function_name: "fn".to_string(),
            mean_duration_us: 1000.0,
            p99_duration_us: 2000,
            call_count: 10,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        let comparison = registry.compare("fn", &platform_a, &platform_b).unwrap();
        assert_eq!(comparison.verdict, ComparisonVerdict::Slower);
    }

    #[test]
    fn test_compare_same() {
        let registry = ComparativeRegistry::new();
        let platform_a = PlatformId {
            signature: "a".to_string(),
            name: "A".to_string(),
        };
        let platform_b = PlatformId {
            signature: "b".to_string(),
            name: "B".to_string(),
        };
        registry.register_benchmark(ComparativeBenchmark {
            platform: platform_a.clone(),
            function_name: "fn".to_string(),
            mean_duration_us: 1000.0,
            p99_duration_us: 2000,
            call_count: 10,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        registry.register_benchmark(ComparativeBenchmark {
            platform: platform_b.clone(),
            function_name: "fn".to_string(),
            mean_duration_us: 1020.0,
            p99_duration_us: 2000,
            call_count: 10,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        let comparison = registry.compare("fn", &platform_a, &platform_b).unwrap();
        assert_eq!(comparison.verdict, ComparisonVerdict::Same);
    }

    #[test]
    fn test_compare_missing_platform() {
        let registry = ComparativeRegistry::new();
        let platform_a = PlatformId {
            signature: "a".to_string(),
            name: "A".to_string(),
        };
        let platform_b = PlatformId {
            signature: "b".to_string(),
            name: "B".to_string(),
        };
        assert!(registry.compare("fn", &platform_a, &platform_b).is_none());
    }

    #[test]
    fn test_with_known_platforms() {
        let registry = ComparativeRegistry::with_known_platforms();
        let platforms = registry.get_platforms();
        assert!(!platforms.is_empty());
        // Should have at least 5 known platforms × 8 reference functions = 40 benchmarks.
        let all_benchmarks: Vec<_> = registry
            .get_platforms()
            .iter()
            .flat_map(|p| registry.get_platform_benchmarks(p))
            .collect();
        assert!(all_benchmarks.len() >= 40);
    }

    #[test]
    fn test_dump_json() {
        let registry = ComparativeRegistry::new();
        let platform = PlatformId {
            signature: "test".to_string(),
            name: "Test".to_string(),
        };
        registry.register_benchmark(ComparativeBenchmark {
            platform,
            function_name: "fn".to_string(),
            mean_duration_us: 100.0,
            p99_duration_us: 200,
            call_count: 1,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        let json = registry.dump_json();
        assert!(json.contains("test"));
        assert!(json.contains("fn"));
    }

    #[test]
    fn test_get_platforms() {
        let registry = ComparativeRegistry::new();
        let p1 = PlatformId {
            signature: "p1".to_string(),
            name: "P1".to_string(),
        };
        let p2 = PlatformId {
            signature: "p2".to_string(),
            name: "P2".to_string(),
        };
        registry.register_benchmark(ComparativeBenchmark {
            platform: p1,
            function_name: "fn".to_string(),
            mean_duration_us: 100.0,
            p99_duration_us: 200,
            call_count: 1,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        registry.register_benchmark(ComparativeBenchmark {
            platform: p2,
            function_name: "fn".to_string(),
            mean_duration_us: 200.0,
            p99_duration_us: 400,
            call_count: 1,
            success_rate: 1.0,
            recorded_at_ms: 0,
            model_id: None,
        });
        let platforms = registry.get_platforms();
        assert_eq!(platforms.len(), 2);
    }
}
