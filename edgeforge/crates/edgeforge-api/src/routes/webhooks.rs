// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # POST /v1/webhooks — Signed Webhook Registration (F-DEV-05)
//!
//! ## Purpose
//! Allows clients to register webhook URLs that receive callbacks when
//! batch jobs complete. Webhook payloads are signed with a secret for
//! verification.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for webhook endpoints
//! - `register_webhook()` — handler for POST /v1/webhooks (register a webhook)
//! - `list_webhooks()` — handler for GET /v1/webhooks (list registered webhooks)
//! - `WebhookRegistration` — request DTO
//! - `WebhookResponse` — response DTO
//! - `WebhookEvent` — events that can trigger webhooks
//! - `generate_webhook_secret()` — generate a random signing secret
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing
//! - `serde` — request/response serialization
//! - `ring` — random secret generation
//!
//! ## Output
//! JSON response with webhook ID and secret.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - webhooks.rs - function - STATUS`

use crate::{middleware::ApiError, state::AppState};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use edgeforge_core::EdgeForgeError;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Events that can trigger webhook callbacks.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    /// Batch job completed successfully.
    JobCompleted,
    /// Batch job failed.
    JobFailed,
    /// Model import completed.
    ModelImported,
    /// Peer joined the mesh.
    PeerJoined,
    /// Peer left the mesh.
    PeerLeft,
}

/// Request body for POST /v1/webhooks.
#[derive(Clone, Debug, Deserialize)]
pub struct WebhookRegistration {
    /// The callback URL to receive webhook payloads.
    pub url: String,
    /// Events that should trigger this webhook.
    pub events: Vec<WebhookEvent>,
}

/// Response body for POST /v1/webhooks.
#[derive(Clone, Debug, Serialize)]
pub struct WebhookResponse {
    /// Unique webhook ID.
    pub id: String,
    /// The callback URL.
    pub url: String,
    /// Events that trigger this webhook.
    pub events: Vec<String>,
    /// The signing secret used to verify webhook payloads.
    /// The client must store this secret; it is used to sign payloads
    /// with HMAC-SHA256.
    pub secret: String,
    /// When the webhook was registered (Unix ms).
    pub created_at: i64,
}

/// Generate a random webhook signing secret.
///
/// ## Purpose
/// Generates a 32-byte random secret encoded as hex. The secret is used
/// to sign webhook payloads with HMAC-SHA256.
///
/// ## Returns
/// `String` — a 64-character hex-encoded secret.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - webhooks.rs - generate_webhook_secret - STATUS`
pub fn generate_webhook_secret() -> String {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-api",
        "webhooks.rs",
        "generate_webhook_secret",
    );
    use ring::rand::SecureRandom;
    let rng = ring::rand::SystemRandom::new();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes).expect("rng failure");
    hex::encode(bytes)
}

/// Returns the axum Router for webhook endpoints.
///
/// ## Returns
/// `Router<AppState>` — the router with GET and POST /v1/webhooks.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - webhooks.rs - routes - STATUS`
pub fn routes() -> Router<AppState> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "webhooks.rs", "routes");
    Router::new()
        .route("/v1/webhooks", get(list_webhooks))
        .route("/v1/webhooks", post(register_webhook))
}

/// Handler for POST /v1/webhooks (register a new webhook).
///
/// ## Purpose
/// Registers a new webhook URL that will receive callbacks for the specified
/// events. Returns a signing secret that the client must store.
///
/// ## Parameters
/// - `state`: The application state.
/// - `req`: The webhook registration request.
///
/// ## Returns
/// `Json<WebhookResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - 400 Bad Request if the URL is empty or no events are specified.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - webhooks.rs - register_webhook - STATUS`
async fn register_webhook(
    State(_state): State<AppState>,
    Json(req): Json<WebhookRegistration>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "webhooks.rs", "register_webhook");

    if req.url.is_empty() {
        guard.error("empty webhook URL");
        return Err(
            EdgeForgeError::InvalidRequest("webhook URL cannot be empty".to_string()).into(),
        );
    }

    if req.events.is_empty() {
        guard.error("no events specified");
        return Err(EdgeForgeError::InvalidRequest(
            "at least one event must be specified".to_string(),
        )
        .into());
    }

    let secret = generate_webhook_secret();
    let events: Vec<String> = req
        .events
        .iter()
        .map(|e| {
            serde_json::to_string(e)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string()
        })
        .collect();

    let response = WebhookResponse {
        id: format!("whk-{}", edgeforge_core::ModelId::new()),
        url: req.url,
        events,
        secret,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    tracing::info!(webhook_id = %response.id, "Webhook registered");
    guard.success();
    Ok(Json(response))
}

/// Handler for GET /v1/webhooks (list registered webhooks).
///
/// ## Purpose
/// Returns a list of all registered webhooks. Phase 11: returns an empty list
/// (webhooks are not yet persisted).
///
/// ## Parameters
/// - `state`: The application state.
///
/// ## Returns
/// `Json<Vec<WebhookResponse>>` — list of registered webhooks.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - webhooks.rs - list_webhooks - STATUS`
async fn list_webhooks(
    State(_state): State<AppState>,
) -> Result<Json<Vec<WebhookResponse>>, ApiError> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "webhooks.rs", "list_webhooks");
    // Phase 11: return empty list (webhooks are not yet persisted).
    // Phase 12+: query from SQLite.
    Ok(Json(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_webhook_secret_length() {
        let secret = generate_webhook_secret();
        assert_eq!(secret.len(), 64); // 32 bytes hex-encoded = 64 chars
    }

    #[test]
    fn test_generate_webhook_secret_is_hex() {
        let secret = generate_webhook_secret();
        assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_webhook_registration_deserialize() {
        let json =
            r#"{"url":"https://example.com/webhook","events":["job_completed","job_failed"]}"#;
        let req: WebhookRegistration = serde_json::from_str(json).unwrap();
        assert_eq!(req.url, "https://example.com/webhook");
        assert_eq!(req.events.len(), 2);
    }

    #[test]
    fn test_webhook_response_serialize() {
        let resp = WebhookResponse {
            id: "whk-123".to_string(),
            url: "https://example.com/webhook".to_string(),
            events: vec!["job_completed".to_string()],
            secret: "abc123".to_string(),
            created_at: 1234567890,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("whk-123"));
        assert!(json.contains("https://example.com/webhook"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_webhook_event_serialize() {
        let event = WebhookEvent::JobCompleted;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"job_completed\"");
    }
}
