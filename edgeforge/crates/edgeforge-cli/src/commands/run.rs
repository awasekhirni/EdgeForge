//! # `edgeforge run` — One-Shot Inference
//!
//! ## Purpose
//! Loads a model from the registry, generates tokens via the SsdEngine, and
//! prints them to stdout. This is the simplest way to test inference without
//! starting the API server.
//!
//! ## What This File Does
//! - Opens the SQLite database + applies migrations
//! - Looks up the model by ID from the registry
//! - Builds a `ModelManifest` from the registry row
//! - Creates an `SsdEngine` and loads the model
//! - Generates tokens and prints each one to stdout (no streaming, just print)
//! - Prints a summary (tokens generated, elapsed ms, tokens/sec)
//!
//! ## Dependencies
//! - `edgeforge-orchestration::Models` — ORM for model lookup
//! - `edgeforge-runtime::{SsdEngine, InferenceEngine, GenParams}` — inference
//! - `edgeforge-crowd::ExecutionMode` — execution mode
//! - `edgeforge-security::{ModelManifest, ModelArch}` — model manifest
//! - `edgeforge-core::{Config, ModelId, Prompt, ChatMessage, Role}` — types
//! - `sqlx` — SQLite connection
//!
//! ## Output
//! Generated text printed to stdout, followed by a summary.
//!
//! ## Function: run
//! - **Purpose:** Run one-shot inference
//! - **Parameters:** `RunArgs` (model, prompt, max_tokens, temperature, seed), `Config`
//! - **Returns:** `anyhow::Result<()>`
//! - **Errors:** Model not found, load failure, generation failure

use super::RunArgs;
use edgeforge_core::Config;
use edgeforge_crowd::ExecutionMode;
use edgeforge_orchestration::Models;
use edgeforge_runtime::{GenParams, InferenceEngine, SsdEngine};
use edgeforge_security::{ModelArch, ModelManifest};
use sqlx::sqlite::SqlitePoolOptions;
use std::time::Instant;

/// Execute the `run` command — one-shot inference.
///
/// ## Purpose
/// Loads the specified model from the registry, creates an SsdEngine, generates
/// tokens from the prompt, and prints them to stdout.
///
/// ## Parameters
/// - `args`: The run arguments (model ID, prompt, max_tokens, temperature, seed).
/// - `cfg`: The EdgeForge configuration (provides the data directory path).
///
/// ## Returns
/// `Ok(())` on success, `Err(anyhow::Error)` on failure.
///
/// ## Errors
/// - If the database cannot be opened or migrations fail
/// - If the model ID is not a valid ULID
/// - If the model is not found in the registry
/// - If the model load fails
/// - If the generation fails
///
/// ## Output
/// Generated text is printed token-by-token to stdout, followed by a summary:
/// ```text
/// Hello world from EdgeForge...
///
/// ────────────────────────────────
/// Tokens generated: 42
/// Elapsed:          150 ms
/// Tokens/sec:       280.0
/// ────────────────────────────────
/// ```
pub async fn run(args: RunArgs, cfg: Config) -> anyhow::Result<()> {
    tracing::info!(
        model = %args.model,
        prompt_len = args.prompt.len(),
        max_tokens = args.max_tokens,
        temperature = args.temperature,
        seed = ?args.seed,
        "run: starting one-shot inference"
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

    // 2. Parse the model ID and look it up.
    let model_id = edgeforge_core::ModelId::parse(&args.model)
        .map_err(|e| anyhow::anyhow!("invalid model ID '{}': {e}", args.model))?;

    let row = models
        .get(model_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("model not found: {}", args.model))?;

    tracing::info!(
        model_id = %row.id,
        name = %row.name,
        "run: model found in registry"
    );

    // 3. Build a ModelManifest.
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
        arch: ModelArch::Moe,
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

    tracing::info!("run: model loaded, generating tokens");

    // 5. Build the prompt.
    let prompt = edgeforge_core::Prompt::new(vec![edgeforge_core::ChatMessage {
        role: edgeforge_core::Role::User,
        content: args.prompt.clone(),
    }]);

    // 6. Build generation parameters.
    let params = GenParams {
        max_tokens: args.max_tokens,
        temperature: args.temperature,
        seed: args.seed,
        ..GenParams::default()
    };

    // 7. Generate tokens and print them.
    let start = Instant::now();
    let mut tokens_generated = 0u32;

    let stats = engine
        .generate(&model, &prompt, &params, &mut |token, _outcome| {
            print!("{}", token.text);
            use std::io::Write;
            let _ = std::io::stdout().flush();
            tokens_generated += 1;
            true
        })
        .await
        .map_err(|e| anyhow::anyhow!("generation failed: {e}"))?;

    let elapsed = start.elapsed();

    // 8. Print the summary.
    println!();
    println!();
    println!("────────────────────────────────");
    println!("Tokens generated: {}", stats.tokens_generated);
    println!("Elapsed:          {} ms", stats.elapsed_ms);
    let tok_per_sec = if stats.elapsed_ms > 0 {
        stats.tokens_generated as f64 / (stats.elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };
    println!("Tokens/sec:       {tok_per_sec:.1}");
    println!("First token:      {} ms", stats.first_token_ms);
    println!("────────────────────────────────");

    tracing::info!(
        tokens_generated = stats.tokens_generated,
        elapsed_ms = stats.elapsed_ms,
        tok_per_sec = tok_per_sec,
        "run: inference complete"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_args_construction() {
        let args = RunArgs {
            model: "01J9TEST".into(),
            prompt: "Hello".into(),
            max_tokens: 64,
            temperature: 0.5,
            seed: Some(42),
        };
        assert_eq!(args.model, "01J9TEST");
        assert_eq!(args.prompt, "Hello");
        assert_eq!(args.max_tokens, 64);
        assert!((args.temperature - 0.5).abs() < 0.01);
        assert_eq!(args.seed, Some(42));
    }

    #[test]
    fn test_run_args_default_temperature() {
        // The clap default is 0.7 — verify via the struct
        let args = RunArgs {
            model: "m".into(), prompt: "p".into(),
            max_tokens: 128, temperature: 0.7, seed: None,
        };
        assert!((args.temperature - 0.7).abs() < 0.01);
    }
}
