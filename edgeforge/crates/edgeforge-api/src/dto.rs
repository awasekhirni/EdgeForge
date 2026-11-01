// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Request/Response DTOs
//!
//! ## Purpose
//! Data Transfer Objects mirroring the OpenAI chat-completions API surface so
//! existing SDKs work as drop-in replacements (F-DEV-02).
//!
//! ## What This File Does
//! - `ChatCompletionRequest` — model, messages, max_tokens, temperature, top_p, stream, stop, seed
//! - `ChatCompletionResponse` — id, object, created, model, choices, usage
//! - `Choice` — index, message, finish_reason
//! - `ChoiceMessage` — role, content
//! - `Usage` — prompt_tokens, completion_tokens, total_tokens
//! - `ModelsResponse` — object, data
//! - `ModelInfo` — id, object, created, owned_by
//! - `HealthResponse` — status, version
//!
//! ## Dependencies
//! - `edgeforge-core::ChatMessage` — for messages field
//! - `serde` — Serialize/Deserialize
//!
//! ## Output
//! Serialized JSON for HTTP responses.

use edgeforge_core::ChatMessage;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize)]
/// `ChatCompletionRequest` — see module documentation for purpose and usage.
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temp")]
    pub temperature: f32,
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub stop: Vec<String>,
    #[serde(default)]
    pub seed: Option<u64>,
}
fn default_max_tokens() -> u32 {
    512
}
fn default_temp() -> f32 {
    0.7
}
fn default_top_p() -> f32 {
    0.9
}
#[derive(Clone, Debug, Serialize)]
/// `ChatCompletionResponse` — see module documentation for purpose and usage.
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}
#[derive(Clone, Debug, Serialize)]
/// `Choice` — see module documentation for purpose and usage.
pub struct Choice {
    pub index: u32,
    pub message: ChoiceMessage,
    pub finish_reason: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
/// `ChoiceMessage` — see module documentation for purpose and usage.
pub struct ChoiceMessage {
    pub role: &'static str,
    pub content: String,
}
#[derive(Clone, Debug, Serialize)]
/// `Usage` — see module documentation for purpose and usage.
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
#[derive(Clone, Debug, Serialize)]
/// `ModelsResponse` — see module documentation for purpose and usage.
pub struct ModelsResponse {
    pub object: &'static str,
    pub data: Vec<ModelInfo>,
}
#[derive(Clone, Debug, Serialize)]
/// `ModelInfo` — see module documentation for purpose and usage.
pub struct ModelInfo {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub owned_by: String,
}
#[derive(Clone, Debug, Serialize)]
/// `HealthResponse` — see module documentation for purpose and usage.
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}
