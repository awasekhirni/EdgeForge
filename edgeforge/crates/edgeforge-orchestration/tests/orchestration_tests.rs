// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed

//! # Orchestration Tests — edgeforge-orchestration
//!
//! Tests for: models CRUD, jobs queue, lifecycle, presets, scheduler, event_bus.

#[cfg(test)]
mod tests {
    use edgeforge_core::{Precision, PrecisionPreset};
    use edgeforge_orchestration::*;
    use sqlx::sqlite::SqlitePoolOptions;

    use std::sync::atomic::{AtomicU64, Ordering};
    static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    async fn setup() -> sqlx::SqlitePool {
        let id = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let db_path =
            std::env::temp_dir().join(format!("edgeforge_test_{}_{}.db", std::process::id(), id));
        let _ = std::fs::remove_file(&db_path);
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();
        // Create tables directly (bypassing migrations for test reliability)
        sqlx::query("CREATE TABLE IF NOT EXISTS models (id TEXT PRIMARY KEY, name TEXT NOT NULL, publisher TEXT NOT NULL, license TEXT NOT NULL, file_path TEXT NOT NULL, file_size_bytes INTEGER NOT NULL, sha256 BLOB NOT NULL, precision TEXT NOT NULL, arch TEXT NOT NULL, total_params INTEGER NOT NULL, active_params INTEGER, dense_core_bytes INTEGER NOT NULL, expert_pool_bytes INTEGER NOT NULL, imported_at INTEGER NOT NULL, last_used_at INTEGER)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE, prompt_hash BLOB NOT NULL, status TEXT NOT NULL, preset TEXT NOT NULL, priority TEXT NOT NULL, params_json TEXT NOT NULL, tokens_generated INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, started_at INTEGER, finished_at INTEGER)")
            .execute(&pool).await.unwrap();
        pool
    }

    // ── Lifecycle Tests ─────────────────────────────────────────────

    #[test]
    fn test_lifecycle_legal_transition() {
        let lc = lifecycle::ModelLifecycle::new();
        let id = edgeforge_core::ModelId::new();
        assert!(lc
            .transition(id, lifecycle::LifecycleState::Loading)
            .is_ok());
        assert!(lc
            .transition(id, lifecycle::LifecycleState::Warming)
            .is_ok());
        assert!(lc
            .transition(id, lifecycle::LifecycleState::Serving)
            .is_ok());
        assert_eq!(lc.state(id), lifecycle::LifecycleState::Serving);
    }

    #[test]
    fn test_lifecycle_illegal_transition() {
        let lc = lifecycle::ModelLifecycle::new();
        let id = edgeforge_core::ModelId::new();
        // Unloaded → Serving is not legal
        assert!(lc
            .transition(id, lifecycle::LifecycleState::Serving)
            .is_err());
    }

    // ── Presets Tests ───────────────────────────────────────────────

    #[test]
    fn test_presets_quality_uses_fp16() {
        let r = presets::resolve(PrecisionPreset::Quality);
        assert_eq!(r.precision, Precision::Fp16);
        assert_eq!(r.prefetch_depth_layers, 1);
        assert_eq!(r.kv_cache_budget_gib, 8);
    }

    #[test]
    fn test_presets_fast_uses_q4() {
        let r = presets::resolve(PrecisionPreset::Fast);
        assert_eq!(r.precision, Precision::Q4);
        assert_eq!(r.prefetch_depth_layers, 2);
    }

    #[test]
    fn test_presets_max_quality_uses_fp32() {
        let r = presets::resolve(PrecisionPreset::MaxQuality);
        assert_eq!(r.precision, Precision::Fp32);
        assert_eq!(r.prefetch_depth_layers, 0);
    }

    // ── Scheduler Tests ─────────────────────────────────────────────

    #[test]
    fn test_scheduler_pick_interactive_first() {
        let mut s = Scheduler::new();
        let j1 = edgeforge_core::JobId::new();
        let j2 = edgeforge_core::JobId::new();
        s.enqueue(j2, JobPriority::Batch);
        s.enqueue(j1, JobPriority::Interactive);
        assert_eq!(s.pick(), Some(j1)); // interactive first
    }

    #[test]
    fn test_scheduler_pick_batch_when_no_interactive() {
        let mut s = Scheduler::new();
        let j1 = edgeforge_core::JobId::new();
        s.enqueue(j1, JobPriority::Batch);
        assert_eq!(s.pick(), Some(j1));
    }

    // ── Event Bus Tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = event_bus::EventBus::new(16);
        let mut rx = bus.subscribe();
        let event = edgeforge_core::ControlEvent {
            timestamp_ms: 42,
            kind: edgeforge_core::ControlEventKind::PeerLost {
                peer: edgeforge_core::NodeId(ulid::Ulid::new()),
            },
        };
        bus.publish(event.clone());
        let received = rx.recv().await.unwrap();
        assert_eq!(received.timestamp_ms, 42);
    }

    #[test]
    fn test_event_bus_subscriber_count() {
        let bus = event_bus::EventBus::new(16);
        assert_eq!(bus.subscriber_count(), 0);
        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    // ── DB / Models Tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_models_insert_and_list() {
        let pool = setup().await;
        let models = Models::new(pool);
        let row = db::NewModelRow {
            name: "Test Model".to_string(),
            publisher: "test".to_string(),
            license: "Apache-2.0".to_string(),
            file_path: std::path::PathBuf::from("/tmp/test.gguf"),
            file_size_bytes: 1024,
            sha256: vec![0u8; 32],
            precision: Precision::Fp16,
            arch: db::ModelArch::Moe,
            total_params: 1_000_000_000,
            active_params: Some(50_000_000),
            dense_core_bytes: 100_000_000,
            expert_pool_bytes: 900_000_000,
        };
        let id = models.insert(&row).await.unwrap();
        let rows = models.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id.to_string());
    }

    #[tokio::test]
    async fn test_models_get_returns_none_for_missing() {
        let pool = setup().await;
        let models = Models::new(pool);
        let id = edgeforge_core::ModelId::new();
        let result = models.get(id).await.unwrap();
        assert!(result.is_none());
    }

    // ── Jobs Tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_jobs_enqueue_and_pick() {
        let pool = setup().await;
        // First insert a model (jobs reference models)
        let models = Models::new(pool.clone());
        let model_id = models
            .insert(&db::NewModelRow {
                name: "Test".to_string(),
                publisher: "test".to_string(),
                license: "Apache-2.0".to_string(),
                file_path: std::path::PathBuf::from("/tmp/test.gguf"),
                file_size_bytes: 1024,
                sha256: vec![0u8; 32],
                precision: Precision::Fp16,
                arch: db::ModelArch::Moe,
                total_params: 1_000_000_000,
                active_params: Some(50_000_000),
                dense_core_bytes: 100_000_000,
                expert_pool_bytes: 900_000_000,
            })
            .await
            .unwrap();

        let jobs = Jobs::new(pool);
        let job_id = jobs
            .enqueue(jobs::EnqueueJob {
                model_id,
                prompt_hash: vec![0u8; 32],
                preset: "quality".to_string(),
                priority: JobPriority::Interactive,
                params_json: "{}".to_string(),
            })
            .await
            .unwrap();

        let picked = jobs.pick_next().await.unwrap().unwrap();
        assert_eq!(picked.id, job_id.to_string());
    }
}
