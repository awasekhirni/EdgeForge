// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # POST /v1/files — RAG File Upload (F-DEV-06)
//!
//! ## Purpose
//! Accepts file uploads for RAG (Retrieval-Augmented Generation) ingestion.
//! Files are chunked, embedded, and stored in the sqlite-vss vector store.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for this endpoint
//! - `upload_file()` — handler for POST /v1/files (multipart upload)
//! - `FileUploadResponse` — response DTO
//! - `FileChunk` — a chunk of a file for embedding
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing, multipart extraction
//! - `serde` — response serialization
//!
//! ## Output
//! JSON response with file ID and chunk count.
//!
//! ## Function Logging
//! `timestamp - edgeforge-api - files.rs - function - STATUS`

use crate::{middleware::ApiError, state::AppState};
use axum::{
    extract::{Multipart, State},
    routing::post,
    Json, Router,
};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::Serialize;
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Default chunk size for file ingestion (512 tokens ≈ ~2KB of text).
const DEFAULT_CHUNK_SIZE: usize = 2048;

/// Response body for POST /v1/files.
#[derive(Clone, Debug, Serialize)]
pub struct FileUploadResponse {
    /// Unique file ID.
    pub id: String,
    /// Status of the upload ("accepted" or "processed").
    pub status: &'static str,
    /// Number of chunks created from the file.
    pub chunks: u32,
    /// File size in bytes.
    pub bytes: u64,
    /// Filename (if provided).
    pub filename: Option<String>,
}

/// A chunk of a file for embedding.
#[derive(Clone, Debug)]
pub struct FileChunk {
    /// Chunk index within the file.
    pub index: u32,
    /// Chunk text content.
    pub content: String,
    /// Source file URI.
    pub source_uri: String,
}

/// Split text into chunks of approximately `chunk_size` characters.
///
/// ## Purpose
/// Splits the input text into chunks at word boundaries, with each chunk
/// approximately `chunk_size` characters long. Used for RAG ingestion.
///
/// ## Parameters
/// - `text`: The text to chunk.
/// - `chunk_size`: Target chunk size in characters.
///
/// ## Returns
/// `Vec<String>` — the text chunks.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - files.rs - chunk_text - STATUS`
pub fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "files.rs", "chunk_text");
    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + chunk_size).min(text.len());
        // Try to break at a word boundary.
        let mut break_point = end;
        if break_point < text.len() {
            // Look backwards for a space.
            for i in (start..end).rev() {
                if text.as_bytes().get(i) == Some(&b' ') {
                    break_point = i + 1;
                    break;
                }
            }
        }
        chunks.push(text[start..break_point].to_string());
        start = break_point;
    }

    chunks
}

/// Returns the axum Router for the files endpoint.
///
/// ## Returns
/// `Router<AppState>` — the router with POST /v1/files.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - files.rs - routes - STATUS`
pub fn routes() -> Router<AppState> {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "files.rs", "routes");
    Router::new().route("/v1/files", post(upload_file))
}

/// Handler for POST /v1/files (multipart file upload).
///
/// ## Purpose
/// Accepts multipart file uploads for RAG ingestion. Files are read,
/// chunked, and (in Phase 12+) embedded and stored in sqlite-vss.
///
/// ## Parameters
/// - `state`: The application state.
/// - `multipart`: The multipart form data.
///
/// ## Returns
/// `Json<FileUploadResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - 400 Bad Request if no file is provided in the multipart data.
/// - 500 Internal Error if file processing fails.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - files.rs - upload_file - STATUS`
async fn upload_file(
    State(_state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<FileUploadResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "files.rs", "upload_file");
    tracing::info!("File upload request received");

    let mut file_content = Vec::new();
    let mut filename: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        edgeforge_core::EdgeForgeError::InvalidRequest(format!("multipart error: {e}"))
    })? {
        // Get the filename from the content disposition.
        if let Some(name) = field.file_name() {
            filename = Some(name.to_string());
        }

        // Read the field content.
        let data = field.bytes().await.map_err(|e| {
            edgeforge_core::EdgeForgeError::InvalidRequest(format!("read error: {e}"))
        })?;
        file_content.extend_from_slice(&data);
    }

    if file_content.is_empty() {
        guard.error("no file content received");
        return Err(
            edgeforge_core::EdgeForgeError::InvalidRequest("no file content".to_string()).into(),
        );
    }

    let file_size = file_content.len() as u64;

    // Convert to text and chunk.
    let text = String::from_utf8_lossy(&file_content);
    let chunks = chunk_text(&text, DEFAULT_CHUNK_SIZE);
    let chunk_count = chunks.len() as u32;

    tracing::info!(
        size = file_size,
        chunks = chunk_count,
        filename = ?filename,
        "File uploaded and chunked"
    );

    // Phase 12+: embed each chunk and store in sqlite-vss.
    // For now, just return the chunk count.

    let response = FileUploadResponse {
        id: format!("file-{}", edgeforge_core::ModelId::new()),
        status: "accepted",
        chunks: chunk_count,
        bytes: file_size,
        filename,
    };

    guard.success();
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_short() {
        let chunks = chunk_text("hello world", 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn test_chunk_text_long() {
        let text = "word ".repeat(1000); // 5000 chars
        let chunks = chunk_text(&text, 100);
        assert!(chunks.len() > 1);
        // Each chunk should be <= 100 chars (approximately).
        for chunk in &chunks {
            assert!(chunk.len() <= 101); // allow 1 char for the trailing space
        }
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("", 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[test]
    fn test_file_upload_response_serialize() {
        let resp = FileUploadResponse {
            id: "file-123".to_string(),
            status: "accepted",
            chunks: 5,
            bytes: 1024,
            filename: Some("test.txt".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("file-123"));
        assert!(json.contains("accepted"));
        assert!(json.contains("test.txt"));
    }
}
