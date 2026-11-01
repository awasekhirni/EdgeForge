//! # `edgeforge bench` — Throughput Benchmark (F-MON-01)
//!
//! ## Purpose
//! Runs a real throughput benchmark using the SsdEngine inference engine.
//! Loads a model from the registry, runs a warmup + measured generation,
//! and prints tokens/sec, first-token latency, and SSD metrics.
//!
//! ## What This File Does
//! - Opens the SQLite database + applies migrations
//! - Looks up the model by ID from the registry
//! - Builds a `ModelManifest` from the registry row
//! - Creates an `SsdEngine` and loads the model
//! - Runs `run_benchmark()` from `edgeforge-orchestration`
//! - Prints a formatted benchmark report to stdout
//!
//! ## Dependencies
//! - `edgeforge-orchestration::{benchmark, Models}` — benchmark harness + ORM
//! - `edgeforge-runtime::{SsdEngine, InferenceEngine}` — inference engine
//! - `edgeforge-crowd::ExecutionMode` — execution mode for model load
//! - `edgeforge-security::{ModelManifest, ModelArch}` — model manifest
//! - `edgeforge-core::{Config, ModelId, Precision}` — types
//! - `sqlx` — SQLite connection
//!
//! ## Output
//! Console output with benchmark results (tokens/sec, latency, SSD metrics).
//!
//! ## Function: run
//! - **Purpose:** Run a throughput benchmark
//! - **Parameters:** `BenchArgs` (model ID, warmup tokens, measured tokens), `Config`
//! - **Returns:** `anyhow::Result<()>`
//! - **Side effects:** Opens SQLite DB; loads model; runs inference
//!
//! ## Error Handling
//! - Returns an error if the model ID is not found in the registry
//! - Returns an error if the DB cannot be opened
//! - Returns an error if the benchmark generation fails
//!
//! ## Logging
//! All major steps are logged via `tracing::info!` / `tracing::error!`.

use super::BenchArgs;
use edgeforge_core::Config;
use edgeforge_crowd::ExecutionMode;
use edgeforge_orchestration::benchmark::{run_benchmark, BenchmarkConfig};
use edgeforge_orchestration::Models;
use edgeforge_runtime::{InferenceEngine, SsdEngine};
use edgeforge_security::{ModelArch, ModelManifest};
use sqlx::sqlite::SqlitePoolOptions;

/// Executes the benchmark command.
///
/// ## Purpose
/// Loads the specified model from the registry, creates an SsdEngine,
/// runs a warmup + measured benchmark, and prints the results.
///
/// ## Parameters
/// - `args`: The benchmark arguments (model ID, warmup tokens, measured tokens).
/// - `cfg`: The EdgeForge configuration (provides the data directory path).
///
/// ## Returns
/// `Ok(())` on success, `Err(anyhow::Error)` on failure.
///
/// ## Errors
/// - If the database cannot be opened or migrations fail
/// - If the model ID is not a valid ULID
/// - If the model is not found in the registry
/// - If the benchmark generation fails
///
/// ## Output
/// Prints a formatted benchmark report to stdout:
/// ```text
/// Benchmark Results
/// ═══════════════════════════════════════
///   Model:            <name>
///   Tokens/sec:       42.5
///   First token:      120 ms
///   Total elapsed:    6020 ms
///   Tokens generated: 256
///   Cold cache:       true
///   SSD read:         1500.0 MB/s
///   SSD temp:         45.0 °C
/// ═══════════════════════════════════════
/// ```
pub async fn run(args: BenchArgs, cfg: Config) -> anyhow::Result<()> {
    tracing::info!(
        model = %args.model,
        warmup = args.warmup,
        tokens = args.tokens,
        "bench: starting benchmark"
    );

    // 1. Open the SQLite database + apply migrations.
    let db_path = cfg.paths.data_dir.join("edgeforge.db");
    if let Some(p) = db_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect(&db_url)
        .await?;
    sqlx::migrate!("../../resources/migrations").run(&pool).await?;
    let models = Models::new(pool);

    // 2. Parse the model ID and look it up in the registry.
    let model_id = edgeforge_core::ModelId::parse(&args.model)
        .map_err(|e| anyhow::anyhow!("invalid model ID '{}': {e}", args.model))?;

    let row = models
        .get(model_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("model not found: {}", args.model))?;

    tracing::info!(
        model_id = %row.id,
        name = %row.name,
        precision = %row.precision,
        "bench: model found in registry"
    );

    // 3. Build a ModelManifest from the registry row.
    let manifest = ModelManifest {
        schema_version: 1,
        id: model_id,
        name: row.name.clone(),
        publisher: row.publisher.clone(),
        license: row.license.clone(),
        license_url: None,
        acceptable_uses: vec![],
        geo_restrictions: vec![],
        file_size_bytes: row.file_size_bytes as u64,
        sha256: row.sha256.clone().try_into().unwrap_or([0u8; 32]),
        precision: row.precision().unwrap_or(edgeforge_core::Precision::Fp16),
        arch: ModelArch::Moe, // Default to MoE; the registry stores the arch as a string
        total_params: row.total_params as u64,
        active_params: row.active_params.map(|v| v as u64),
        dense_core_bytes: row.dense_core_bytes as u64,
        expert_pool_bytes: row.expert_pool_bytes as u64,
        signature: None,
        publisher_key_fingerprint: None,
    };

    // 4. Create the SsdEngine and load the model.
    let engine = SsdEngine::new();
    let model = engine
        .load(&manifest, ExecutionMode::Local)
        .await
        .map_err(|e| anyhow::anyhow!("failed to load model: {e}"))?;

    tracing::info!("bench: model loaded, starting benchmark");

    // 5. Run the benchmark.
    let bench_config = BenchmarkConfig {
        model: model_id,
        preset: "balanced".to_string(),
        warmup_tokens: args.warmup,
        measured_tokens: args.tokens,
    };

    let result = run_benchmark(&engine, &model, &bench_config)
        .await
        .map_err(|e| anyhow::anyhow!("benchmark failed: {e}"))?;

    // 6. Print the results.
    println!();
    println!("Benchmark Results");
    println!("═══════════════════════════════════════");
    println!("  Model:            {}", row.name);
    println!("  Tokens/sec:       {:.1}", result.tokens_per_sec.0);
    println!("  First token:      {} ms", result.first_token_ms);
    println!("  Total elapsed:    {} ms", result.elapsed_ms);
    println!("  Tokens generated: {}", bench_config.measured_tokens);
    println!("  Cold cache:       {}", result.cold_cache);
    if let Some(mbps) = result.ssd_read_mbps {
        println!("  SSD read:         {mbps:.1} MB/s");
    }
    if let Some(temp) = result.ssd_temp_celsius {
        println!("  SSD temp:         {temp:.1} °C");
    }
    println!("═══════════════════════════════════════");
    println!();

    tracing::info!(
        tok_per_sec = %result.tokens_per_sec,
        first_token_ms = result.first_token_ms,
        elapsed_ms = result.elapsed_ms,
        "bench: benchmark complete"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_args_construction() {
        let args = BenchArgs {
            model: "01J9TEST".into(),
            warmup: 16,
            tokens: 64,
        };
        assert_eq!(args.model, "01J9TEST");
        assert_eq!(args.warmup, 16);
        assert_eq!(args.tokens, 64);
    }
}
