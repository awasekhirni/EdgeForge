//! # Throughput Benchmark Harness (F-MON-01)
//!
//! ## Purpose
//! Runs a timed generation benchmark to measure tokens per second (tok/s).
//! Records cold-cache vs warm-cache results and SSD thermal state.
//!
//! ## What This File Does
//! - `BenchmarkConfig` — benchmark parameters (model, preset, warmup, tokens)
//! - `BenchmarkResult` — benchmark results (tok/s, latency, SSD metrics)
//! - `run_benchmark()` — run a benchmark using an InferenceEngine
//!
//! ## Dependencies
//! - `edgeforge-core` — ModelId, TokPerSec
//! - `edgeforge-runtime` — InferenceEngine, GenParams, LoadedModel
//! - `edgeforge-crowd` — ExecutionMode
//! - `edgeforge-security` — ModelManifest
//! - `edgeforge-observability` — FunctionGuard for call logging
//!
//! ## Output
//! `BenchmarkResult` with tok/s, first-token latency, and SSD metrics.
//!
//! ## Function Logging
//! `timestamp - edgeforge-orchestration - benchmark.rs - function - STATUS`

use edgeforge_core::{ModelId, Prompt, TokPerSec};
use edgeforge_crowd::ExecutionMode;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use edgeforge_runtime::{GenParams, InferenceEngine, LoadedModel};
use edgeforge_security::ModelManifest;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Instant;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Benchmark run configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// The model to benchmark.
    pub model: ModelId,
    /// The preset to use ("quality", "balanced", "fast", "max_quality").
    pub preset: String,
    /// Number of warmup tokens (not measured).
    pub warmup_tokens: u32,
    /// Number of measured tokens.
    pub measured_tokens: u32,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            model: ModelId::new(),
            preset: "quality".to_string(),
            warmup_tokens: 32,
            measured_tokens: 256,
        }
    }
}

/// Benchmark run result. Includes the extended metrics from G-06.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Tokens per second.
    pub tokens_per_sec: TokPerSec,
    /// Time to first token in milliseconds.
    pub first_token_ms: u64,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// SSD read bandwidth in MB/s (if available).
    pub ssd_read_mbps: Option<f32>,
    /// SSD temperature in Celsius (if available).
    pub ssd_temp_celsius: Option<f32>,
    /// Whether this was a cold-cache run.
    pub cold_cache: bool,
    /// Speculation acceptance rate (0.0–1.0, if available).
    pub speculation_acceptance_rate: Option<f32>,
    /// Prefetch hit rate (0.0–1.0, if available).
    pub prefetch_hit_rate: Option<f32>,
    /// Surrogate accuracy delta (if available).
    pub surrogate_accuracy_delta: Option<f32>,
    /// Crowd tier (if applicable).
    pub crowd_tier: Option<String>,
    /// Verification pending count.
    pub verification_pending_count: u32,
    /// Verification corrected count.
    pub verification_corrected_count: u32,
    /// Energy per token in joules (ADR-026).
    pub energy_joules_per_token: Option<f32>,
    /// Carbon grams per 1k tokens (ADR-026).
    pub carbon_grams_per_1k_tokens: Option<f32>,
    /// Grid region (ADR-026).
    pub region: Option<String>,
}

/// Run a benchmark using an InferenceEngine.
///
/// ## Purpose
/// Runs a warmup phase (not measured) followed by a measured phase.
/// Returns comprehensive benchmark results including tok/s, latency,
/// and SSD metrics.
///
/// ## Parameters
/// - `engine`: The inference engine to benchmark.
/// - `model`: The loaded model.
/// - `config`: The benchmark configuration.
///
/// ## Returns
/// `Ok(BenchmarkResult)` on success, `Err(EdgeForgeError)` on failure.
///
/// ## Errors
/// - `Engine` if the generation fails.
///
/// ## Function Logging
/// `timestamp - edgeforge-orchestration - benchmark.rs - run_benchmark - STATUS`
pub async fn run_benchmark(
    engine: &dyn InferenceEngine,
    model: &LoadedModel,
    config: &BenchmarkConfig,
) -> Result<BenchmarkResult, edgeforge_core::EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-orchestration",
        "benchmark.rs",
        "run_benchmark",
    );
    tracing::info!(
        model = %model.id,
        preset = %config.preset,
        warmup = config.warmup_tokens,
        measured = config.measured_tokens,
        "Starting benchmark"
    );

    // Warmup phase (not measured).
    if config.warmup_tokens > 0 {
        let warmup_prompt = Prompt::new(vec![edgeforge_core::ChatMessage {
            role: edgeforge_core::Role::User,
            content: "Warmup: The quick brown fox jumps over the lazy dog.".to_string(),
        }]);
        let warmup_params = GenParams {
            max_tokens: config.warmup_tokens,
            ..GenParams::default()
        };
        let _ = engine
            .generate(model, &warmup_prompt, &warmup_params, &mut |_, _| true)
            .await;
    }

    // Measured phase.
    let prompt = Prompt::new(vec![edgeforge_core::ChatMessage {
        role: edgeforge_core::Role::User,
        content: "Benchmark: In a hole in the ground there lived a hobbit.".to_string(),
    }]);
    let params = GenParams {
        max_tokens: config.measured_tokens,
        ..GenParams::default()
    };

    let start = Instant::now();
    let stats = engine
        .generate(model, &prompt, &params, &mut |_, _| true)
        .await?;
    let elapsed = start.elapsed();

    let tok_per_sec = if elapsed.as_secs_f32() > 0.0 {
        TokPerSec(stats.tokens_generated as f32 / elapsed.as_secs_f32())
    } else {
        TokPerSec(0.0)
    };

    let result = BenchmarkResult {
        tokens_per_sec: tok_per_sec,
        first_token_ms: stats.first_token_ms,
        elapsed_ms: stats.elapsed_ms,
        ssd_read_mbps: if stats.ssd_read_mbps > 0.0 { Some(stats.ssd_read_mbps) } else { None },
        ssd_temp_celsius: if stats.ssd_temp_celsius > 0.0 { Some(stats.ssd_temp_celsius) } else { None },
        cold_cache: stats.cold_cache,
        speculation_acceptance_rate: None,
        prefetch_hit_rate: None,
        surrogate_accuracy_delta: None,
        crowd_tier: None,
        verification_pending_count: 0,
        verification_corrected_count: 0,
        energy_joules_per_token: None,
        carbon_grams_per_1k_tokens: None,
        region: None,
    };

    tracing::info!(
        tok_per_sec = %result.tokens_per_sec,
        first_token_ms = result.first_token_ms,
        elapsed_ms = result.elapsed_ms,
        cold_cache = result.cold_cache,
        "Benchmark complete"
    );
    guard.success();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_runtime::MockInferenceEngine;
    use edgeforge_security::{ModelArch, ModelManifest};

    fn make_manifest() -> ModelManifest {
        ModelManifest {
            schema_version: 1,
            id: ModelId::new(),
            name: "Benchmark Test".to_string(),
            publisher: "test".to_string(),
            license: "Apache-2.0".to_string(),
            license_url: None,
            acceptable_uses: vec![],
            geo_restrictions: vec![],
            file_size_bytes: 1024,
            sha256: [0u8; 32],
            precision: edgeforge_core::Precision::Fp16,
            arch: ModelArch::Moe,
            total_params: 1_000_000_000,
            active_params: Some(50_000_000),
            dense_core_bytes: 100_000_000,
            expert_pool_bytes: 900_000_000,
            signature: None,
            publisher_key_fingerprint: None,
        }
    }

    #[tokio::test]
    async fn test_run_benchmark_with_mock_engine() {
        let engine = MockInferenceEngine::new();
        let manifest = make_manifest();
        let model = engine.load(&manifest, ExecutionMode::Local).await.unwrap();
        let config = BenchmarkConfig {
            warmup_tokens: 4,
            measured_tokens: 8,
            ..Default::default()
        };
        let result = run_benchmark(&engine, &model, &config).await.unwrap();
        assert!(result.tokens_per_sec.0 >= 0.0);
        // elapsed_ms may be 0 on very fast machines; just check it's valid
        assert!(result.elapsed_ms < 60_000); // less than a minute
    }

    #[test]
    fn test_benchmark_config_default() {
        let c = BenchmarkConfig::default();
        assert_eq!(c.warmup_tokens, 32);
        assert_eq!(c.measured_tokens, 256);
        assert_eq!(c.preset, "quality");
    }
}
