// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # SsdEngine — Custom Rust SSD-Streaming Engine (ADR-032 Implementation B)
//!
//! ## Purpose
//! Fallback inference engine implemented in pure Rust. Spec-compliant with
//! edgeforge-core-legacy contracts. Used when llama.cpp is unavailable or
//! incompatible with future hardware.
//!
//! ## What This File Does
//! - `SsdEngine` — implements `InferenceEngine` using the weight pager +
//!   expert cache + PlatformIo backend
//! - `load()` — loads model metadata, builds the SSD expert index
//! - `generate()` — streams expert weights from SSD, computes via the
//!   custom inference loop
//!
//! ## Dependencies
//! - `edgeforge-storage::{WeightPager, ExpertCache, PlatformIo}`
//! - `edgeforge-core` — Prompt, Token
//! - `edgeforge-crowd` — ExecutionMode, VerificationOutcome
//! - `edgeforge-security` — ModelManifest
//!
//! ## Output
//! `GenStats` — tokens_generated, elapsed_ms, first_token_ms.
//!
//! ## Dual Implementation (ADR-032)
//! Both LlamaBackend (Impl A) and SsdEngine (Impl B) implement the same
//! InferenceEngine trait and pass the same test suite. Selection is
//! re-evaluated annually via formal DAR criteria.

use crate::engine::{EngineError, GenParams, GenStats, InferenceEngine, LoadedModel};
use crate::router_hook::RouterHook;
use async_trait::async_trait;
use edgeforge_core::{Prompt, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_security::ModelManifest;
use std::sync::{Arc, Mutex};

/// Custom Rust SSD-streaming inference engine.
///
/// This engine does not depend on llama.cpp. It uses the EdgeForge weight
/// pager + expert cache + PlatformIo backend to stream expert weights from
/// SSD and compute inference in pure Rust.
///
/// Phase 8: scaffolding. The real inference loop will be implemented in
/// Phase 9+ once the PlatformIo backends are production-ready.
pub struct SsdEngine {
    router_hook: Mutex<Option<Box<dyn RouterHook>>>,
}

impl SsdEngine {
    /// Create a new SsdEngine.
    ///
    /// ## Returns
    /// `Ok(SsdEngine)` — always succeeds (no external dependencies).
    pub fn new() -> Self {
        Self {
            router_hook: Mutex::new(None),
        }
    }
}

impl Default for SsdEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceEngine for SsdEngine {
    /// Load a model's metadata and build the SSD expert index.
    ///
    /// ## Parameters
    /// - `manifest`: The model manifest
    /// - `mode`: Execution mode
    ///
    /// ## Returns
    /// `Ok(LoadedModel)` on success.
    ///
    /// ## Errors
    /// `NotImplemented` — Phase 8 stub.
    async fn load(
        &self,
        manifest: &ModelManifest,
        _mode: ExecutionMode,
    ) -> Result<LoadedModel, EngineError> {
        tracing::info!(model = %manifest.name, "SsdEngine: model loaded (scaffolding)");
        Ok(LoadedModel {
            id: manifest.id,
            manifest: Arc::new(manifest.clone()),
        })
    }

    /// Generate tokens using the custom SSD-streaming inference loop.
    ///
    /// ## Parameters
    /// - `model`: The loaded model
    /// - `prompt`: The user's prompt
    /// - `params`: Generation parameters
    /// - `on_token`: Callback invoked per token
    ///
    /// ## Returns
    /// `GenStats` on success.
    ///
    /// ## Errors
    /// `NotImplemented` — Phase 8 stub. The real implementation will:
    /// 1. Tokenize the prompt
    /// 2. For each layer, stream experts from SSD via WeightPager
    /// 3. Run the attention + FFN computation
    /// 4. Sample the next token
    /// 5. Invoke the on_token callback
    async fn generate(
        &self,
        _model: &LoadedModel,
        _prompt: &Prompt,
        _params: &GenParams,
        _on_token: &mut (dyn FnMut(Token, edgeforge_crowd::VerificationOutcome) -> bool + Send),
    ) -> Result<GenStats, EngineError> {
        Err(EngineError::NotImplemented(
            "SsdEngine::generate — Phase 9+ (custom Rust inference loop)",
        ))
    }

    fn install_router_hook(&self, hook: Box<dyn RouterHook>) -> Result<(), EngineError> {
        let mut guard = self
            .router_hook
            .lock()
            .map_err(|e| EngineError::Engine(format!("router hook lock poisoned: {e}")))?;
        *guard = Some(hook);
        Ok(())
    }
}
