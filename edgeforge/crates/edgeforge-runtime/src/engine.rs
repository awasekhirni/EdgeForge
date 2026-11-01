// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Inference Engine Trait
//!
//! ## Purpose
//! The L4 abstraction. L5 talks only to this trait, never to the FFI.
//! `LlamaBackend` and `MockInferenceEngine` both implement this trait.
//!
//! ## What This File Does
//! - `LoadedModel` — loaded model (id + manifest)
//! - `GenParams` — max_tokens, temperature, top_p, seed, stop_sequences
//! - `GenStats` — tokens_generated, elapsed_ms, first_token_ms, ssd metrics
//! - `InferenceEngine` trait — load, generate, install_router_hook
//!
//! ## Dependencies
//! - `edgeforge-core::{EdgeForgeError, ModelId, Prompt, Token}`
//! - `edgeforge-crowd::{ExecutionMode, VerificationOutcome}`
//! - `edgeforge-security::ModelManifest`
//!
//! ## Output
//! `Result<LoadedModel, EngineError>` from load; `Result<GenStats, EngineError>` from generate.
//!
//! ## Function: generate (trait)
//! - **Purpose:** Stream tokens from the model
//! - **Parameters:** `&LoadedModel`, `&Prompt`, `&GenParams`, callback `FnMut(Token, VerificationOutcome) -> bool`
//! - **Returns:** `Result<GenStats, EngineError>` — statistics from the generation
//! - **Callback:** Called per token; return `false` to abort generation
//! - **VerificationOutcome:** NotRequired for Local mode; Verified/Pending/Corrected for Tier 2/3
//! - **Dependencies:** Calls router_hook.resolve_expert() per token (if installed)

use async_trait::async_trait;
use edgeforge_core::{EdgeForgeError, ModelId, Prompt, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_security::ModelManifest;
use std::sync::Arc;
/// Type alias for `EngineError`.
pub type EngineError = EdgeForgeError;
/// `LoadedModel` — see module documentation for purpose and usage.
pub struct LoadedModel {
    pub id: ModelId,
    pub manifest: Arc<ModelManifest>,
}
#[derive(Clone, Debug)]
/// `GenParams` — see module documentation for purpose and usage.
pub struct GenParams {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub seed: Option<u64>,
    pub stop_sequences: Vec<String>,
}
impl Default for GenParams {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.9,
            seed: None,
            stop_sequences: Vec::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
/// `GenStats` — see module documentation for purpose and usage.
pub struct GenStats {
    pub tokens_generated: u32,
    pub elapsed_ms: u64,
    pub first_token_ms: u64,
    pub ssd_read_mbps: f32,
    pub ssd_temp_celsius: f32,
    pub cold_cache: bool,
}
#[async_trait]
/// The `InferenceEngine` trait — see module documentation for the contract.
pub trait InferenceEngine: Send + Sync + 'static {
    async fn load(
        &self,
        manifest: &ModelManifest,
        mode: ExecutionMode,
    ) -> Result<LoadedModel, EngineError>;
    async fn generate(
        &self,
        model: &LoadedModel,
        prompt: &Prompt,
        params: &GenParams,
        on_token: &mut (dyn FnMut(Token, edgeforge_crowd::VerificationOutcome) -> bool + Send),
    ) -> Result<GenStats, EngineError>;
    fn install_router_hook(
        &self,
        hook: Box<dyn crate::router_hook::RouterHook>,
    ) -> Result<(), EngineError>;
}
