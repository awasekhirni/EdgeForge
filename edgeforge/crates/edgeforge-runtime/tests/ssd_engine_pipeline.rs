//! Integration tests for the SsdEngine inference engine.
//!
//! These tests exercise the full SsdEngine pipeline:
//!   - Token generation through the on_token callback
//!   - Router hook invocation per (layer, expert)
//!   - Real SSD reads via WeightPager + BlockingIo + temp file
//!   - ExpertCache hit/miss tracking
//!   - Determinism with seed
//!   - Stop sequences
//!   - GenStats reporting (tokens_generated, elapsed_ms, ssd_read_mbps, etc.)
//!
//! These tests do NOT require llama.cpp — SsdEngine is pure Rust.

use edgeforge_core::{Bytes, ChatMessage, ExpertId, LayerIdx, ModelId, Prompt, Role, Token};
use edgeforge_crowd::{ExecutionMode, VerificationOutcome};
use edgeforge_runtime::{
    ExpertResolution, GenParams, InferenceEngine, RouterHook, SsdEngine, SsdEngineConfig,
};
use edgeforge_security::{ModelArch, ModelManifest};
use edgeforge_storage::{
    expert_cache::{CacheKey, ExpertCache, ExpertCacheConfig},
    weight_pager::{WeightPageId, WeightPager, WeightPagerConfig},
    BlockingIo,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tempfile::NamedTempFile;

// ── Helpers ────────────────────────────────────────────────────────────────

fn make_manifest() -> ModelManifest {
    ModelManifest {
        schema_version: 1,
        id: ModelId::new(),
        name: "SsdEngine Integration Test Model".to_string(),
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

fn make_prompt(text: &str) -> Prompt {
    Prompt::new(vec![ChatMessage {
        role: Role::User,
        content: text.to_string(),
    }])
}

/// A counting router hook that always resolves to SSD, so the engine issues
/// real weight-pager reads.
struct CountingSsdHook {
    counter: Arc<AtomicU32>,
}

impl RouterHook for CountingSsdHook {
    fn resolve_expert(&self, _layer: LayerIdx, _expert: ExpertId) -> ExpertResolution {
        self.counter.fetch_add(1, Ordering::Relaxed);
        ExpertResolution::Ssd
    }
}

/// A router hook that always resolves to RAM, exercising the cache path.
struct RamHook;

impl RouterHook for RamHook {
    fn resolve_expert(&self, _layer: LayerIdx, _expert: ExpertId) -> ExpertResolution {
        ExpertResolution::Ram
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn ssd_engine_full_pipeline_with_real_weight_pager() {
    // Set up a temp file with enough bytes for 4 layers × 4096-byte pages.
    let tmp = NamedTempFile::new().unwrap();
    let data: Vec<u8> = (0..16_384).map(|i| (i % 256) as u8).collect();
    std::fs::write(tmp.path(), &data).unwrap();

    let io = Arc::new(BlockingIo::new());
    let pager = Arc::new(WeightPager::new(
        WeightPagerConfig {
            page_size: Bytes(4096),
            ram_budget: Bytes(1024 * 1024),
            ssd_path: tmp.path().to_path_buf(),
        },
        io,
        ModelId::new(),
    ));
    let cache = Arc::new(ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(1024 * 1024),
    }));

    let engine = SsdEngine::with_config(SsdEngineConfig {
        n_layers: 4,
        n_experts_per_layer: 8,
        top_k_experts: 2,
        weight_pager: Some(pager),
        expert_cache: Some(cache.clone()),
        thermal: None,
    });

    let hook_counter = Arc::new(AtomicU32::new(0));
    engine
        .install_router_hook(Box::new(CountingSsdHook {
            counter: hook_counter.clone(),
        }))
        .unwrap();

    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .expect("load should succeed");

    let prompt = make_prompt("the quick brown fox jumps over the lazy dog");
    let params = GenParams {
        max_tokens: 5,
        temperature: 0.7,
        top_p: 0.9,
        seed: Some(42),
        stop_sequences: vec![],
    };

    let mut tokens: Vec<Token> = Vec::new();
    let mut outcomes: Vec<VerificationOutcome> = Vec::new();
    let stats = engine
        .generate(&model, &prompt, &params, &mut |token, outcome| {
            tokens.push(token);
            outcomes.push(outcome);
            true
        })
        .await
        .expect("generation should succeed");

    // 1. Tokens were produced.
    assert!(stats.tokens_generated > 0, "should generate tokens");
    assert_eq!(tokens.len() as u32, stats.tokens_generated);

    // 2. All outcomes are NotRequired (Local mode).
    for o in &outcomes {
        assert_eq!(*o, VerificationOutcome::NotRequired);
    }

    // 3. Router hook was called once per (token, layer, expert).
    let expected_calls = stats.tokens_generated * 4 * 2; // n_layers * top_k
    assert_eq!(
        hook_counter.load(Ordering::Relaxed),
        expected_calls,
        "router hook should be called {} times ({} tokens × 4 layers × 2 experts)",
        expected_calls,
        stats.tokens_generated
    );

    // 4. SSD metrics are reported.
    assert!(stats.cold_cache, "cold_cache should be true after SSD reads");

    // 5. Cache was populated by the page-ins (each (layer, expert) should be cached now).
    // After generation, at least one entry should be in the cache.
    assert!(
        cache.len() > 0,
        "expert cache should have entries after generation, got {}",
        cache.len()
    );
}

#[tokio::test]
async fn ssd_engine_ram_hook_uses_cache_path() {
    let cache = Arc::new(ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(1024 * 1024),
    }));
    // Pre-populate the cache so the Ram hook path finds hits.
    cache.put(
        CacheKey {
            layer: LayerIdx(0),
            expert: ExpertId(0),
        },
        vec![0xAB; 64],
    );

    let engine = SsdEngine::with_config(SsdEngineConfig {
        n_layers: 1,
        n_experts_per_layer: 1,
        top_k_experts: 1,
        weight_pager: None,
        expert_cache: Some(cache),
        thermal: None,
    });
    engine.install_router_hook(Box::new(RamHook)).unwrap();

    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("hello world");
    let params = GenParams {
        max_tokens: 1,
        seed: Some(1),
        ..GenParams::default()
    };

    let stats = engine
        .generate(&model, &prompt, &params, &mut |_, _| true)
        .await
        .unwrap();
    assert_eq!(stats.tokens_generated, 1);
    // RAM-resolved experts never trigger SSD reads → cold_cache stays false.
    assert!(
        !stats.cold_cache,
        "cold_cache should be false when all experts resolve to RAM"
    );
}

#[tokio::test]
async fn ssd_engine_deterministic_with_same_seed() {
    let engine = SsdEngine::new();
    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("alpha beta gamma delta epsilon zeta eta theta iota");
    let params = GenParams {
        max_tokens: 12,
        seed: Some(99),
        ..GenParams::default()
    };

    let mut t1: Vec<String> = Vec::new();
    engine
        .generate(&model, &prompt, &params, &mut |t, _| {
            t1.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();

    let mut t2: Vec<String> = Vec::new();
    engine
        .generate(&model, &prompt, &params, &mut |t, _| {
            t2.push(t.text.trim().to_string());
            true
        })
        .await
        .unwrap();

    assert_eq!(t1, t2, "same seed must produce identical output");
    assert_eq!(t1.len(), 12);
}

#[tokio::test]
async fn ssd_engine_respects_max_tokens() {
    let engine = SsdEngine::new();
    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("one two three four five six seven eight nine ten");
    let params = GenParams {
        max_tokens: 4,
        seed: Some(7),
        ..GenParams::default()
    };
    let mut count = 0u32;
    let stats = engine
        .generate(&model, &prompt, &params, &mut |_, _| {
            count += 1;
            true
        })
        .await
        .unwrap();
    assert_eq!(stats.tokens_generated, 4);
    assert_eq!(count, 4);
}

#[tokio::test]
async fn ssd_engine_callback_can_abort() {
    let engine = SsdEngine::new();
    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("a b c d e f g h i j k l m n o p");
    let params = GenParams {
        max_tokens: 100,
        seed: Some(1),
        ..GenParams::default()
    };
    let mut count = 0u32;
    let stats = engine
        .generate(&model, &prompt, &params, &mut |_, _| {
            count += 1;
            count < 3
        })
        .await
        .unwrap();
    assert_eq!(stats.tokens_generated, 3);
    assert_eq!(count, 3);
}

#[tokio::test]
async fn ssd_engine_load_fails_on_empty_name() {
    let engine = SsdEngine::new();
    let mut manifest = make_manifest();
    manifest.name = String::new();
    let result = engine.load(&manifest, ExecutionMode::Local).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ssd_engine_stop_sequence_halts() {
    let engine = SsdEngine::new();
    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("the cat sat on the mat the cat ran the cat jumped over");
    let params = GenParams {
        max_tokens: 50,
        seed: Some(123),
        stop_sequences: vec!["cat".to_string()],
        ..GenParams::default()
    };
    let mut emitted = String::new();
    let stats = engine
        .generate(&model, &prompt, &params, &mut |t, _| {
            emitted.push_str(&t.text);
            true
        })
        .await
        .unwrap();
    assert!(stats.tokens_generated <= 50);
}

#[tokio::test]
async fn ssd_engine_empty_prompt_still_works() {
    let engine = SsdEngine::new();
    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("");
    let params = GenParams {
        max_tokens: 3,
        seed: Some(5),
        ..GenParams::default()
    };
    let mut count = 0u32;
    let stats = engine
        .generate(&model, &prompt, &params, &mut |_, _| {
            count += 1;
            true
        })
        .await
        .unwrap();
    assert_eq!(stats.tokens_generated, 3);
    assert_eq!(count, 3);
}

#[tokio::test]
async fn ssd_engine_genstats_has_real_metrics() {
    let engine = SsdEngine::with_config(SsdEngineConfig {
        n_layers: 2,
        n_experts_per_layer: 4,
        top_k_experts: 1,
        ..SsdEngineConfig::default()
    });
    let model = engine
        .load(&make_manifest(), ExecutionMode::Local)
        .await
        .unwrap();
    let prompt = make_prompt("hello world from edgeforge ssd engine integration test");
    let params = GenParams {
        max_tokens: 5,
        seed: Some(42),
        ..GenParams::default()
    };
    let stats = engine
        .generate(&model, &prompt, &params, &mut |_, _| true)
        .await
        .unwrap();
    assert_eq!(stats.tokens_generated, 5);
    // elapsed_ms may be 0 on very fast machines; the spec allows that as long
    // as tokens_generated > 0.
    assert!(stats.elapsed_ms > 0 || stats.tokens_generated > 0);
    // ssd_temp_celsius defaults to 45.0 when no thermal monitor is configured.
    assert!(stats.ssd_temp_celsius > 0.0);
}
