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
pub struct ChatCompletionRequest { pub model: String, pub messages: Vec<ChatMessage>, #[serde(default = "default_max_tokens")] pub max_tokens: u32, #[serde(default = "default_temp")] pub temperature: f32, #[serde(default = "default_top_p")] pub top_p: f32, #[serde(default)] pub stream: bool, #[serde(default)] pub stop: Vec<String>, #[serde(default)] pub seed: Option<u64> }
fn default_max_tokens() -> u32 { 512 }
fn default_temp() -> f32 { 0.7 }
fn default_top_p() -> f32 { 0.9 }
#[derive(Clone, Debug, Serialize)]
/// `ChatCompletionResponse` — see module documentation for purpose and usage.
pub struct ChatCompletionResponse { pub id: String, pub object: &'static str, pub created: i64, pub model: String, pub choices: Vec<Choice>, pub usage: Usage }
#[derive(Clone, Debug, Serialize)]
/// `Choice` — see module documentation for purpose and usage.
pub struct Choice { pub index: u32, pub message: ChoiceMessage, pub finish_reason: Option<String> }
#[derive(Clone, Debug, Serialize)]
/// `ChoiceMessage` — see module documentation for purpose and usage.
pub struct ChoiceMessage { pub role: &'static str, pub content: String }
#[derive(Clone, Debug, Serialize)]
/// `Usage` — see module documentation for purpose and usage.
pub struct Usage { pub prompt_tokens: u32, pub completion_tokens: u32, pub total_tokens: u32 }
#[derive(Clone, Debug, Serialize)]
/// `ModelsResponse` — see module documentation for purpose and usage.
pub struct ModelsResponse { pub object: &'static str, pub data: Vec<ModelInfo> }
#[derive(Clone, Debug, Serialize)]
/// `ModelInfo` — see module documentation for purpose and usage.
pub struct ModelInfo { pub id: String, pub object: &'static str, pub created: i64, pub owned_by: String }
#[derive(Clone, Debug, Serialize)]
/// `HealthResponse` — see module documentation for purpose and usage.
pub struct HealthResponse { pub status: &'static str, pub version: &'static str }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_defaults() {
        let json = r#"{"model":"test","messages":[]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_tokens, 512);
        assert!((req.temperature - 0.7).abs() < 0.01);
        assert!((req.top_p - 0.9).abs() < 0.01);
        assert!(!req.stream);
        assert!(req.stop.is_empty());
        assert!(req.seed.is_none());
    }

    #[test]
    fn test_chat_request_with_all_fields() {
        let json = r#"{"model":"m1","messages":[],"max_tokens":100,"temperature":0.5,"top_p":0.8,"stream":true,"stop":["\n"],"seed":42}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_tokens, 100);
        assert!((req.temperature - 0.5).abs() < 0.01);
        assert!(req.stream);
        assert_eq!(req.stop, vec!["\n"]);
        assert_eq!(req.seed, Some(42));
    }

    #[test]
    fn test_chat_response_serializes() {
        let resp = ChatCompletionResponse {
            id: "chatcmpl-1".into(), object: "chat.completion", created: 0,
            model: "test".into(),
            choices: vec![Choice { index: 0, message: ChoiceMessage { role: "assistant", content: "hi".into() }, finish_reason: Some("stop".into()) }],
            usage: Usage { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("chat.completion"));
        assert!(json.contains("assistant"));
    }

    #[test]
    fn test_models_response_serializes() {
        let resp = ModelsResponse { object: "list", data: vec![ModelInfo { id: "01J9".into(), object: "model", created: 0, owned_by: "test".into() }] };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"list\""));
        assert!(json.contains("01J9"));
    }

    #[test]
    fn test_health_response_serializes() {
        let resp = HealthResponse { status: "ok", version: "0.1.0" };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ok"));
        assert!(json.contains("0.1.0"));
    }

    #[test]
    fn test_usage_fields() {
        let u = Usage { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 };
        let json = serde_json::to_string(&u).unwrap();
        assert!(json.contains("10"));
        assert!(json.contains("20"));
        assert!(json.contains("30"));
    }

    #[test]
    fn test_choice_message_role() {
        let m = ChoiceMessage { role: "assistant", content: "hello".into() };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("assistant"));
        assert!(json.contains("hello"));
    }

    #[test]
    fn test_choice_with_null_finish_reason() {
        let c = Choice { index: 0, message: ChoiceMessage { role: "assistant", content: "hi".into() }, finish_reason: None };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("null"));
    }
}
