//! # edgeforge-runtime — L4 Inference Runtime
//!
//! ## Purpose
//! The Rust facade over llama.cpp (via edgeforge-runtime-sys) plus the custom
//! SSD-streaming engine. L5 (edgeforge-orchestration) talks only to the
//! InferenceEngine trait defined here — never to the FFI.
//!
//! ## Modules
//! - `engine` — InferenceEngine trait, GenParams, GenStats, LoadedModel
//! - `mock_engine` — MockInferenceEngine for end-to-end testing (no llama.cpp needed)
//! - `llama_backend` — Real llama.cpp backend (ADR-032 Implementation A)
//! - `ssd_engine` — Custom Rust SSD-streaming engine (ADR-032 Implementation B).
//!   **Working out of the box** — pure-Rust, no FFI, never returns NotImplemented.
//! - `embedding` — EmbeddingEngine trait + HashingEmbedder (RAG vector embeddings, F-DEV-06)
//! - `router_hook` — RouterHook trait + GraphAwareRouterHook + ExpertResolution
//! - `speculation` — Multi-horizon speculation engine (H1/H2/H3, ADR-015/016)
//! - `batching` — Continuous batching for multi-user throughput (F-SCHED-03)
//! - `priority` — Interactive vs batch priority scheduler (F-SCHED-04)
//! - `determinism` — Reproducible mode with fixed seed (F-SCHED-05)
//! - `profiler` — Per-layer latency profiler (F-MON-05)
//!
//! ## Dependencies
//! - `edgeforge-core` — Prompt, Token, EdgeForgeError
//! - `edgeforge-security` — ModelManifest
//! - `edgeforge-storage` — ExpertCache, ThermalState, WeightPager
//! - `edgeforge-graph` — StreamingGraph (for GraphAwareRouterHook)
//! - `edgeforge-crowd` — ExecutionMode, VerificationOutcome
//! - `edgeforge-runtime-sys` — FFI bindings (stub or real)
//! - `parking_lot` — Mutex for SsdEngine router hook + storage RwLocks
//!
//! ## Output
//! `GenStats` — tokens_generated, elapsed_ms, first_token_ms, SSD metrics.
//! `Vec<f32>` — L2-normalised embedding vectors from the embedding module.
//!
//! ## Engine Selection
//! | Engine | Feature Flag | Requires llama.cpp | Status |
//! |---|---|---|---|
//! | `SsdEngine` | (default) | No | ✅ Working (pure-Rust, deterministic) |
//! | `MockInferenceEngine` | (default) | No | ✅ Working (test stub, reverses prompt) |
//! | `LlamaBackend` | `llama-backend` | Yes | ✅ Real-mode loop implemented; needs llama.cpp build |
//! | `HashingEmbedder` | (default) | No | ✅ Working (pure-Rust feature-hashing embeddings) |

pub mod batching;
pub mod determinism;
pub mod embedding;
pub mod engine;
pub mod llama_backend;
pub mod mock_engine;
pub mod priority;
pub mod profiler;
pub mod router_hook;
pub mod speculation;
pub mod ssd_engine;

pub use determinism::{
    effective_seed, global_seed, is_deterministic, reset_determinism, set_determinism,
    DeterminismConfig,
};
pub use embedding::{
    cosine_similarity, decode_vector_f32le, dot_product, encode_vector_f32le, l2_norm,
    EmbeddingEngine, HashingEmbedder, HashingEmbedderConfig,
};
pub use engine::{EngineError, GenParams, GenStats, InferenceEngine, LoadedModel};
pub use llama_backend::LlamaBackend;
pub use mock_engine::MockInferenceEngine;
pub use router_hook::{ExpertResolution, GraphAwareRouterHook, RamOnlyRouterHook, RouterHook, SsdOnlyRouterHook};
pub use speculation::{permit_surrogate, detect_workload, strategy_for_workload, SpeculationEngine, SpeculationStrategy, WorkloadType};
pub use ssd_engine::{SsdEngine, SsdEngineConfig};
