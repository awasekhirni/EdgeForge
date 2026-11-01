// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-api — OpenAI-Compatible REST API (ADR-007)
//!
//! ## Purpose
//! Axum-based HTTP server exposing OpenAI-compatible endpoints at
//! `127.0.0.1:11434`. Supports SSE streaming for chat completions.
//!
//! ## Modules
//! - `server` — axum app builder + serve()
//! - `state` — AppState (shared Models + Jobs)
//! - `dto` — request/response types (serde)
//! - `routes/` — chat, completions, embeddings, models, health
//! - `middleware/` — error, rate_limit, auth, logging, verification
//!
//! ## Dependencies
//! - `axum` — HTTP framework
//! - `edgeforge-orchestration` — Models, Jobs
//! - `edgeforge-runtime` — InferenceEngine, MockInferenceEngine
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
