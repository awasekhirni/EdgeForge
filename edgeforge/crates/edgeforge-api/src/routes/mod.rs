// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Route Handlers
//!
//! ## Purpose
//! Declares all route handler modules for the EdgeForge API.
//!
//! ## Modules
//! - `chat` — POST /v1/chat/completions (SSE + JSON)
//! - `completions` — POST /v1/completions (text completion)
//! - `embeddings` — POST /v1/embeddings (RAG embeddings, F-DEV-06)
//! - `files` — POST /v1/files (RAG file upload, F-DEV-06)
//! - `health` — GET /health
//! - `models` — GET /v1/models, GET /v1/models/:id
//! - `webhooks` — GET/POST /v1/webhooks (signed callbacks, F-DEV-05)

pub mod chat;
pub mod completions;
pub mod embeddings;
pub mod files;
pub mod health;
pub mod models;
pub mod webhooks;
