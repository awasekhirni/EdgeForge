//! # CLI Integration Tests — edgeforge-cli
//!
//! Tests for the CLI command modules:
//! - `bench::run()` — runs a real benchmark against a model in the registry
//! - `config_cmd::run()` — prints config as JSON
//! - `model::run()` — model list on a fresh DB (empty list)
//! - `serve::run()` — AppState construction (doesn't bind a real listener)
//!
//! These tests use a temp-dir SQLite database to avoid touching the user's
//! real data directory.

#[cfg(test)]
mod tests {
    use edgeforge_cli::commands::{
        bench::run as bench_run,
        config_cmd::run as config_run,
        model::run as model_run,
        BenchArgs, ConfigArgs, ModelArgs, ModelCommand,
    };
    use edgeforge_core::Config;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::TempDir;

    /// Create a Config with a temp data_dir so we don't touch the user's data.
    fn make_config() -> (Config, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            paths: edgeforge_core::PathsConfig {
                data_dir: dir.path().to_path_buf(),
                model_dir: dir.path().join("models"),
                cache_dir: dir.path().join("cache"),
                log_dir: dir.path().join("logs"),
            },
            ..Default::default()
        };
        (cfg, dir)
    }

    /// Apply migrations to the temp DB so model operations work.
    async fn apply_migrations(cfg: &Config) {
        let db_path = cfg.paths.data_dir.join("edgeforge.db");
        std::fs::create_dir_all(&cfg.paths.data_dir).unwrap();
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();
        sqlx::migrate!("../../resources/migrations")
            .run(&pool)
            .await
            .unwrap();
    }

    /// Insert a test model into the registry and return its ID.
    async fn insert_test_model(cfg: &Config) -> String {
        let db_path = cfg.paths.data_dir.join("edgeforge.db");
        let db_url = format!("sqlite://{}", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();
        let models = edgeforge_orchestration::Models::new(pool);
        let model_id = models
            .insert(&edgeforge_orchestration::NewModelRow {
                name: "Test Model".to_string(),
                publisher: "test".to_string(),
                license: "Apache-2.0".to_string(),
                file_path: std::path::PathBuf::from("/tmp/test.gguf"),
                file_size_bytes: 1024,
                sha256: vec![0u8; 32],
                precision: edgeforge_core::Precision::Fp16,
                arch: edgeforge_orchestration::ModelArch::Moe,
                total_params: 1_000_000_000,
                active_params: Some(50_000_000),
                dense_core_bytes: 100_000_000,
                expert_pool_bytes: 900_000_000,
            })
            .await
            .unwrap();
        model_id.to_string()
    }

    // ── bench::run tests ──

    #[tokio::test]
    async fn test_bench_runs_benchmark() {
        // Previously: bench::run() printed "stub" and returned Ok.
        // Now: it loads the model, runs a real benchmark, and prints results.
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;
        let model_id = insert_test_model(&cfg).await;

        let args = BenchArgs {
            model: model_id,
            warmup: 4,
            tokens: 8,
        };

        // Run the benchmark — should succeed (SsdEngine produces real tokens).
        let result = bench_run(args, cfg).await;
        assert!(
            result.is_ok(),
            "bench::run should succeed with a valid model ID, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_bench_invalid_model_id() {
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;

        let args = BenchArgs {
            model: "not-a-valid-ulid".to_string(),
            warmup: 4,
            tokens: 8,
        };

        let result = bench_run(args, cfg).await;
        assert!(result.is_err(), "bench should fail with an invalid model ID");
    }

    #[tokio::test]
    async fn test_bench_model_not_found() {
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;

        // Use a valid ULID that doesn't exist in the registry.
        let model_id = edgeforge_core::ModelId::new().to_string();
        let args = BenchArgs {
            model: model_id,
            warmup: 4,
            tokens: 8,
        };

        let result = bench_run(args, cfg).await;
        assert!(result.is_err(), "bench should fail when model not found");
    }

    // ── config_cmd::run tests ──

    #[tokio::test]
    async fn test_config_json_output() {
        let (cfg, _dir) = make_config();
        let args = ConfigArgs { json: true };

        // Should succeed and print JSON.
        let result = config_run(args, cfg).await;
        assert!(result.is_ok(), "config --json should succeed");
    }

    #[tokio::test]
    async fn test_config_toml_output() {
        let (cfg, _dir) = make_config();
        let args = ConfigArgs { json: false };

        // Should succeed and print TOML.
        let result = config_run(args, cfg).await;
        assert!(result.is_ok(), "config (TOML) should succeed");
    }

    // ── model::run tests ──

    #[tokio::test]
    async fn test_model_list_empty() {
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;

        let args = ModelArgs {
            command: ModelCommand::List,
        };

        // Should succeed — empty list is a valid state.
        let result = model_run(args, cfg).await;
        assert!(result.is_ok(), "model list should succeed on empty DB");
    }

    #[tokio::test]
    async fn test_model_list_after_import() {
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;
        let _model_id = insert_test_model(&cfg).await;

        let args = ModelArgs {
            command: ModelCommand::List,
        };

        let result = model_run(args, cfg).await;
        assert!(result.is_ok(), "model list should succeed after import");
    }

    #[tokio::test]
    async fn test_model_verify_existing() {
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;
        let model_id = insert_test_model(&cfg).await;

        let args = ModelArgs {
            command: ModelCommand::Verify {
                id: model_id.clone(),
            },
        };

        let result = model_run(args, cfg).await;
        assert!(result.is_ok(), "model verify should succeed for an existing model");
    }

    #[tokio::test]
    async fn test_model_verify_not_found() {
        let (cfg, _dir) = make_config();
        apply_migrations(&cfg).await;

        let args = ModelArgs {
            command: ModelCommand::Verify {
                id: "01J9XXXXXXXXXXXXXXXXXXX".to_string(),
            },
        };

        let result = model_run(args, cfg).await;
        assert!(result.is_err(), "model verify should fail for non-existent model");
    }

    // ── serve::run tests (don't bind a real listener, just verify config) ──

    #[tokio::test]
    async fn test_serve_config_validation() {
        // We can't test serve::run() directly because it binds a TCP listener,
        // but we can verify the config loads correctly (which serve uses).
        let (cfg, _dir) = make_config();
        assert!(cfg.validate().is_ok(), "config should validate");
        assert_eq!(cfg.api.listen_addr, "127.0.0.1:11434");
    }
}
