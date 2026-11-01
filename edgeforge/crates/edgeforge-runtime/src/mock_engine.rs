// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Mock Inference Engine (Phase 3)
//!
//! ## Purpose
//! Implements `InferenceEngine` without requiring the real `llama.cpp` FFI build.
//! Produces deterministic token output based on the prompt, allowing the full
//! pipeline (router hook → weight pager → KV cache → SSE streaming) to be
//! exercised in tests.
//!
//! ## What This File Does
//! - `MockInferenceEngine` — implements InferenceEngine
//! - `load()` — validates manifest, returns LoadedModel
//! - `generate()` — derives deterministic response (reversed prompt), calls
//!   router_hook per token, invokes on_token callback
//! - `install_router_hook()` — stores hook for use during generate
//! - `generate_mock_response()` — reverses prompt words, repeats to fill max_tokens
//!
//! ## Dependencies
//! - `edgeforge-core::{Prompt, Token}`
//! - `edgeforge-crowd::{ExecutionMode, VerificationOutcome}`
//! - `edgeforge-security::ModelManifest`
//! - `std::sync::Mutex` — for router hook storage
//! - `std::time::Instant` — for timing
//!
//! ## Output
//! `GenStats` — tokens_generated, elapsed_ms, first_token_ms.
//!
//! ## Function: generate
//! - **Purpose:** Generate tokens deterministically from the prompt
//! - **Parameters:** model, prompt, params, on_token callback
//! - **Returns:** `Result<GenStats, EngineError>`
//! - **Algorithm:** Reverses prompt words, repeats to fill max_tokens
//! - **Router hook:** Calls resolve_expert(layer=i, expert=i%8) per token
//! - **Verification:** Emits VerificationOutcome::NotRequired for Local mode
//! - **Abort:** If callback returns false, generation stops
//!
//! ## Note
//! In production, `LlamaBackend` (ADR-032 Implementation A) replaces this mock.
//! The mock is retained in the test suite for integration testing.

use crate::engine::{EngineError, GenParams, GenStats, InferenceEngine, LoadedModel};
use crate::router_hook::RouterHook;
use async_trait::async_trait;
use edgeforge_core::{Prompt, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_security::ModelManifest;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Mock inference engine for end-to-end pipeline testing (Phase 3).
/// Produces deterministic tokens derived from the prompt — no llama.cpp needed.
pub struct MockInferenceEngine {
    router_hook: Mutex<Option<Box<dyn RouterHook>>>,
}
impl MockInferenceEngine {
    pub fn new() -> Self {
        Self {
            router_hook: Mutex::new(None),
        }
    }
}
impl Default for MockInferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceEngine for MockInferenceEngine {
    async fn load(
        &self,
        manifest: &ModelManifest,
        _mode: ExecutionMode,
    ) -> Result<LoadedModel, EngineError> {
        if manifest.name.is_empty() {
            return Err(EngineError::ModelLoadFailed {
                model: manifest.id.to_string(),
                reason: "empty name".into(),
            });
        }
        tracing::info!(model = %manifest.name, "mock engine: model loaded");
        Ok(LoadedModel {
            id: manifest.id,
            manifest: Arc::new(manifest.clone()),
        })
    }
    async fn generate(
        &self,
        model: &LoadedModel,
        prompt: &Prompt,
        params: &GenParams,
        on_token: &mut (dyn FnMut(Token, edgeforge_crowd::VerificationOutcome) -> bool + Send),
    ) -> Result<GenStats, EngineError> {
        let start = Instant::now();
        let prompt_text = prompt.render_flat();
        let response = generate_mock_response(&prompt_text, params.max_tokens);
        let mut tokens_generated = 0u32;
        let mut first_token_ms = 0u64;
        for (i, word) in response.split_whitespace().enumerate() {
            if i == 0 {
                first_token_ms = start.elapsed().as_millis() as u64;
            }
            let token = Token {
                text: format!("{word} "),
                id: i as u32,
            };
            tokens_generated += 1;
            if let Ok(guard) = self.router_hook.lock() {
                if let Some(hook) = guard.as_ref() {
                    let _ = hook.resolve_expert(
                        edgeforge_core::LayerIdx(i as u32),
                        edgeforge_core::ExpertId((i % 8) as u32),
                    );
                }
            }
            let outcome = edgeforge_crowd::VerificationOutcome::NotRequired;
            if !on_token(token, outcome) {
                break;
            }
        }
        let elapsed_ms = start.elapsed().as_millis() as u64;
        tracing::info!(model = %model.manifest.name, tokens = tokens_generated, elapsed_ms, "mock engine: generation complete");
        Ok(GenStats {
            tokens_generated,
            elapsed_ms,
            first_token_ms,
            ssd_read_mbps: 0.0,
            ssd_temp_celsius: 45.0,
            cold_cache: true,
        })
    }
    fn install_router_hook(&self, hook: Box<dyn RouterHook>) -> Result<(), EngineError> {
        let mut guard = self
            .router_hook
            .lock()
            .map_err(|e| EngineError::Engine(format!("lock poisoned: {e}")))?;
        *guard = Some(hook);
        Ok(())
    }
}

fn generate_mock_response(prompt: &str, max_tokens: u32) -> String {
    let words: Vec<&str> = prompt.split_whitespace().collect();
    if words.is_empty() {
        return "Hello from EdgeForge mock engine.".into();
    }
    let reversed: Vec<&str> = words.iter().rev().copied().collect();
    let mut result: Vec<&str> = Vec::new();
    while (result.len() as u32) < max_tokens {
        result.extend(reversed.iter());
    }
    result
        .into_iter()
        .take(max_tokens as usize)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::{ChatMessage, ModelId, Role};
    use edgeforge_security::{ModelArch, ModelManifest};
    fn make_manifest() -> ModelManifest {
        ModelManifest {
            schema_version: 1,
            id: ModelId::new(),
            name: "Mock Test Model".into(),
            publisher: "test".into(),
            license: "Apache-2.0".into(),
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
            content: text.into(),
        }])
    }
    #[tokio::test]
    async fn load_succeeds() {
        let e = MockInferenceEngine::new();
        let m = e
            .load(&make_manifest(), ExecutionMode::Local)
            .await
            .unwrap();
        assert_eq!(m.manifest.name, "Mock Test Model");
    }
    #[tokio::test]
    async fn generate_produces_tokens() {
        let e = MockInferenceEngine::new();
        let m = e
            .load(&make_manifest(), ExecutionMode::Local)
            .await
            .unwrap();
        let p = make_prompt("hello world from edgeforge");
        let params = GenParams {
            max_tokens: 10,
            ..GenParams::default()
        };
        let mut tokens = Vec::new();
        let stats = e
            .generate(&m, &p, &params, &mut |t, _| {
                tokens.push(t);
                true
            })
            .await
            .unwrap();
        assert!(stats.tokens_generated > 0);
        assert!(!tokens.is_empty());
    }
    #[tokio::test]
    async fn respects_max_tokens() {
        let e = MockInferenceEngine::new();
        let m = e
            .load(&make_manifest(), ExecutionMode::Local)
            .await
            .unwrap();
        let p = make_prompt("one two three four five six seven eight nine ten");
        let params = GenParams {
            max_tokens: 3,
            ..GenParams::default()
        };
        let mut count = 0u32;
        let stats = e
            .generate(&m, &p, &params, &mut |_, _| {
                count += 1;
                true
            })
            .await
            .unwrap();
        assert_eq!(stats.tokens_generated, 3);
        assert_eq!(count, 3);
    }
    #[tokio::test]
    async fn aborts_on_false() {
        let e = MockInferenceEngine::new();
        let m = e
            .load(&make_manifest(), ExecutionMode::Local)
            .await
            .unwrap();
        let p = make_prompt("a b c d e f g h i j");
        let params = GenParams {
            max_tokens: 100,
            ..GenParams::default()
        };
        let mut count = 0u32;
        let stats = e
            .generate(&m, &p, &params, &mut |_, _| {
                count += 1;
                count < 3
            })
            .await
            .unwrap();
        // The callback returns false on the 3rd token (count=3, 3<3 is false),
        // but the token was already emitted before the return value is checked.
        // So 3 tokens are generated, and the loop breaks after the 3rd.
        assert_eq!(stats.tokens_generated, 3);
    }
    #[tokio::test]
    async fn load_fails_on_empty_name() {
        let e = MockInferenceEngine::new();
        let mut m = make_manifest();
        m.name = "".into();
        assert!(e.load(&m, ExecutionMode::Local).await.is_err());
    }
    #[test]
    fn mock_response_reverses() {
        let r = generate_mock_response("hello world", 5);
        let w: Vec<&str> = r.split_whitespace().collect();
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "world");
        assert_eq!(w[1], "hello");
    }
}
