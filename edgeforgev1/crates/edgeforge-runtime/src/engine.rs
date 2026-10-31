// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use async_trait::async_trait;
use edgeforge_core::{EdgeForgeError, ModelId, Prompt, Token};
use edgeforge_crowd::ExecutionMode;
use edgeforge_security::ModelManifest;
use std::sync::Arc;
pub type EngineError = EdgeForgeError;
pub struct LoadedModel {
    pub id: ModelId,
    pub manifest: Arc<ModelManifest>,
}
#[derive(Clone, Debug)]
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
pub struct GenStats {
    pub tokens_generated: u32,
    pub elapsed_ms: u64,
    pub first_token_ms: u64,
    pub ssd_read_mbps: f32,
    pub ssd_temp_celsius: f32,
    pub cold_cache: bool,
}
#[async_trait]
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
