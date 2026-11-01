//! # edgeforge-api — OpenAI-Compatible REST API (ADR-007)
//!
//! ## Purpose
//! Axum-based HTTP server exposing OpenAI-compatible endpoints at
//! `127.0.0.1:11434`. Supports SSE streaming for chat completions.
//!
//! ## Modules
//! - `server` — axum app builder + serve()
//! - `state` — AppState (shared Models + Jobs + Embeddings + Embedder)
//! - `dto` — request/response types (serde)
//! - `routes/` — chat, completions, embeddings, files, models, health, webhooks
//! - `middleware/` — error, rate_limit, auth, logging, verification
//!
//! ## Endpoints
//! | Method | Path | Purpose |
//! |---|---|---|
//! | GET  | /health | Health check |
//! | GET  | /v1/models | List imported models |
//! | GET  | /v1/models/:id | Get a model by ID |
//! | POST | /v1/chat/completions | Chat completion (SSE streaming supported) |
//! | POST | /v1/completions | Legacy text completion |
//! | POST | /v1/embeddings | Generate embedding vectors |
//! | POST | /v1/embeddings/search | RAG similarity search |
//! | POST | /v1/files | RAG file upload (chunk + embed + store) |
//! | GET/POST | /v1/webhooks | Webhook management |
//!
//! ## Dependencies
//! - `axum` — HTTP framework
//! - `edgeforge-orchestration` — Models, Jobs, Embeddings
//! - `edgeforge-runtime` — SsdEngine, EmbeddingEngine, HashingEmbedder
//! - `edgeforge-crowd` — VerificationOutcome (for X-EdgeForge-Verification header)
//! - `tokio-stream` + `futures` — SSE body stream
//!
//! ## Output
//! HTTP responses (JSON or SSE stream).

pub mod dto;
pub mod middleware;
pub mod routes;
pub mod server;
pub mod state;
pub use server::{build_router, serve};
pub use state::AppState;
