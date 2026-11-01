//! # Shared Application State
//!
//! ## Purpose
//! Top-level state shared by all axum handlers. Cheap to clone (all Arc).
//!
//! ## What This File Does
//! - `AppState` — Models, Jobs, Embeddings, Embedder, RateLimiter,
//!   require_api_key, api_key, rate_limit_per_minute
//! - `new()` — constructor
//! - `with_embedder()` — constructor with custom embedder
//! - `with_auth()` — constructor with explicit auth config (used by tests)
//!
//! ## Security
//! `require_api_key` + `api_key` drive the `auth_middleware`. When
//! `require_api_key` is true, the `Authorization: Bearer <key>` header is
//! validated against `api_key` using a constant-time comparison. When
//! `api_key` is `None` but `require_api_key` is true, any bearer token is
//! accepted (useful for testing).
//!
//! `rate_limiter` is shared across all requests and enforces per-IP
//! token-bucket rate limiting at `rate_limit_per_minute` requests/minute.
//!
//! ## Dependencies
//! - `edgeforge-orchestration::{Models, Jobs, Embeddings}`
//! - `edgeforge-runtime::{EmbeddingEngine, HashingEmbedder}`
//! - `crate::middleware::rate_limit::RateLimiter`
//! - `std::sync::Arc`
//!
//! ## Output
//! `AppState` — cloned per request by axum.

use crate::middleware::rate_limit::RateLimiter;
use edgeforge_orchestration::{Embeddings, Jobs, Models};
use edgeforge_runtime::{EmbeddingEngine, HashingEmbedder};
use parking_lot::RwLock;
use std::sync::Arc;

/// A stored webhook registration (persisted in-memory in AppState).
#[derive(Clone, Debug, serde::Serialize)]
pub struct StoredWebhook {
    /// Unique webhook ID.
    pub id: String,
    /// The callback URL.
    pub url: String,
    /// Events that trigger this webhook (as strings).
    pub events: Vec<String>,
    /// The signing secret (HMAC-SHA256).
    pub secret: String,
    /// When the webhook was registered (Unix ms).
    pub created_at: i64,
}

/// Thread-safe in-memory webhook store.
#[derive(Debug, Default)]
pub struct WebhookStore {
    inner: RwLock<Vec<StoredWebhook>>,
}

impl WebhookStore {
    /// Creates a new empty webhook store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Vec::new()),
        }
    }

    /// Add a webhook to the store.
    pub fn add(&self, webhook: StoredWebhook) {
        self.inner.write().push(webhook);
    }

    /// Returns all stored webhooks.
    #[must_use]
    pub fn list(&self) -> Vec<StoredWebhook> {
        self.inner.read().clone()
    }

    /// Returns the number of stored webhooks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns true if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Shared application state, cloned per request by axum.
///
/// All fields are `Arc`-wrapped so cloning is cheap. The embedder is stored
/// as `Arc<dyn EmbeddingEngine>` so handlers can swap implementations
/// (e.g. a real neural embedder when the `llama-backend` feature is on).
#[derive(Clone)]
pub struct AppState {
    /// Model registry ORM.
    pub models: Arc<Models>,
    /// Job queue ORM.
    pub jobs: Arc<Jobs>,
    /// RAG embeddings vector store ORM.
    pub embeddings: Arc<Embeddings>,
    /// The embedding engine used by `/v1/embeddings` and `/v1/files`.
    pub embedder: Arc<dyn EmbeddingEngine>,
    /// Per-IP token-bucket rate limiter (shared across all requests).
    pub rate_limiter: Arc<RateLimiter>,
    /// In-memory webhook store (registered webhooks).
    pub webhooks: Arc<WebhookStore>,
    /// Whether API key authentication is required.
    pub require_api_key: bool,
    /// The expected API key (None = any bearer token accepted when required).
    pub api_key: Option<String>,
    /// Rate limit (requests per minute per client).
    pub rate_limit_per_minute: u32,
}

impl AppState {
    /// Create a new `AppState` with the default `HashingEmbedder`.
    ///
    /// ## Parameters
    /// - `models`: The model registry ORM.
    /// - `jobs`: The job queue ORM.
    /// - `embeddings`: The RAG embeddings ORM.
    /// - `require_api_key`: Whether API key auth is required.
    /// - `api_key`: The expected API key (None = any bearer token accepted).
    /// - `rate_limit_per_minute`: Rate limit (req/min per client).
    ///
    /// ## Returns
    /// `Self` — a cheaply-clonable state.
    #[must_use]
    pub fn new(
        models: Models,
        jobs: Jobs,
        embeddings: Embeddings,
        require_api_key: bool,
        api_key: Option<String>,
        rate_limit_per_minute: u32,
    ) -> Self {
        Self::with_embedder(
            models,
            jobs,
            embeddings,
            Arc::new(HashingEmbedder::new()),
            require_api_key,
            api_key,
            rate_limit_per_minute,
        )
    }

    /// Create a new `AppState` with a custom embedding engine.
    ///
    /// Use this when you want to swap in a real neural embedder (e.g. a GGUF
    /// embedding model via LlamaBackend).
    ///
    /// ## Parameters
    /// - `models`: The model registry ORM.
    /// - `jobs`: The job queue ORM.
    /// - `embeddings`: The RAG embeddings ORM.
    /// - `embedder`: The embedding engine.
    /// - `require_api_key`: Whether API key auth is required.
    /// - `api_key`: The expected API key (None = any bearer token accepted).
    /// - `rate_limit_per_minute`: Rate limit (req/min per client).
    #[must_use]
    pub fn with_embedder(
        models: Models,
        jobs: Jobs,
        embeddings: Embeddings,
        embedder: Arc<dyn EmbeddingEngine>,
        require_api_key: bool,
        api_key: Option<String>,
        rate_limit_per_minute: u32,
    ) -> Self {
        // The rate limiter's burst capacity is the per-minute limit (so a
        // client can briefly burst up to the full per-minute quota), and the
        // refill rate is the per-minute limit (so the bucket refills
        // continuously over the minute).
        let rate_limiter = Arc::new(RateLimiter::new(
            rate_limit_per_minute.max(1),
            rate_limit_per_minute.max(1),
        ));

        tracing::info!(
            embedder_model_id = %embedder.model_id(),
            embedder_dim = embedder.dim(),
            require_api_key,
            has_api_key = api_key.is_some(),
            rate_limit_per_minute,
            "AppState: initialized"
        );
        Self {
            models: Arc::new(models),
            jobs: Arc::new(jobs),
            embeddings: Arc::new(embeddings),
            embedder,
            rate_limiter,
            webhooks: Arc::new(WebhookStore::new()),
            require_api_key,
            api_key,
            rate_limit_per_minute,
        }
    }

    /// Returns a reference to the shared rate limiter.
    #[must_use]
    pub fn rate_limiter(&self) -> &Arc<RateLimiter> {
        &self.rate_limiter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_store_new_is_empty() {
        let store = WebhookStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_webhook_store_add_and_list() {
        let store = WebhookStore::new();
        let webhook = StoredWebhook {
            id: "whk-1".into(),
            url: "https://example.com/hook".into(),
            events: vec!["job_completed".into()],
            secret: "abc123".into(),
            created_at: 123,
        };
        store.add(webhook.clone());
        assert_eq!(store.len(), 1);
        let list = store.list();
        assert_eq!(list[0].id, "whk-1");
    }

    #[test]
    fn test_webhook_store_multiple_adds() {
        let store = WebhookStore::new();
        for i in 0..5 {
            store.add(StoredWebhook {
                id: format!("whk-{i}"),
                url: format!("https://example.com/{i}"),
                events: vec![],
                secret: "x".into(),
                created_at: i,
            });
        }
        assert_eq!(store.len(), 5);
        assert_eq!(store.list().len(), 5);
    }
}
