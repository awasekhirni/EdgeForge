//! # POST /v1/files — RAG File Upload (F-DEV-06)
//!
//! ## Purpose
//! Accepts file uploads for RAG (Retrieval-Augmented Generation) ingestion.
//! Files are chunked, embedded using the configured `EmbeddingEngine`, and
//! stored in the `embedding_sources` + `embedding_vectors` tables so they can
//! be retrieved via `POST /v1/embeddings/search`.
//!
//! ## What This File Does
//! - `routes()` — returns the axum Router for this endpoint
//! - `upload_file()` — handler for POST /v1/files (multipart upload)
//! - `FileUploadResponse` — response DTO
//! - `FileChunk` — a chunk of a file for embedding
//! - `chunk_text()` — splits text into word-boundary chunks (public, tested)
//!
//! ## Pipeline
//! 1. Read the multipart file content
//! 2. Convert to UTF-8 text
//! 3. Chunk into ~2KB pieces at word boundaries
//! 4. Embed each chunk using the configured `EmbeddingEngine`
//! 5. Store each chunk + vector in `embedding_sources` + `embedding_vectors`
//! 6. Return the file ID + chunk count
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError
//! - `edgeforge-runtime` — EmbeddingEngine
//! - `edgeforge-orchestration` — Embeddings ORM, StoreChunkParams
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `axum` — HTTP routing, multipart extraction
//! - `serde` — response serialization
//!
//! ## Output
//! JSON response with file ID, chunk count, and stored vector count.
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
use edgeforge_orchestration::StoreChunkParams;
use serde::Serialize;
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Default chunk size for file ingestion (≈2KB of text ≈ 500 tokens).
const DEFAULT_CHUNK_SIZE: usize = 2048;

/// Response body for POST /v1/files.
#[derive(Clone, Debug, Serialize)]
pub struct FileUploadResponse {
    /// Unique file ID.
    pub id: String,
    /// Status of the upload ("processed" — chunks were embedded and stored).
    pub status: &'static str,
    /// Number of chunks created from the file.
    pub chunks: u32,
    /// Number of vectors successfully stored (may be < chunks if some
    /// embedding/storage operations failed).
    pub stored: u32,
    /// File size in bytes.
    pub bytes: u64,
    /// Filename (if provided).
    pub filename: Option<String>,
    /// The embedding model ID used.
    pub model_id: String,
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
    if chunk_size == 0 {
        return vec![text.to_string()];
    }
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
        // Ensure we always make progress (avoid infinite loop on a single
        // very long word with no spaces).
        if break_point <= start {
            break_point = end;
        }
        chunks.push(text[start..break_point].to_string());
        start = break_point;
    }

    chunks
}

/// Returns the axum Router for the files endpoint.
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
/// chunked, embedded, and stored in the `embedding_sources` +
/// `embedding_vectors` tables so they can be retrieved via
/// `POST /v1/embeddings/search`.
///
/// ## Parameters
/// - `state`: The application state (contains the embedder + embeddings ORM).
/// - `multipart`: The multipart form data.
///
/// ## Returns
/// `Json<FileUploadResponse>` on success, or `ApiError` on failure.
///
/// ## Errors
/// - 400 Bad Request if no file is provided in the multipart data.
/// - 500 Internal Error if file processing or embedding fails.
///
/// ## Function Logging
/// `timestamp - edgeforge-api - files.rs - upload_file - STATUS`
async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<FileUploadResponse>, ApiError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-api", "files.rs", "upload_file");
    tracing::info!("File upload request received");

    let mut file_content = Vec::new();
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| edgeforge_core::EdgeForgeError::InvalidRequest(format!("multipart error: {e}")))?
    {
        // Get the filename from the content disposition.
        if let Some(name) = field.file_name() {
            filename = Some(name.to_string());
        }

        // Read the field content.
        let data = field
            .bytes()
            .await
            .map_err(|e| edgeforge_core::EdgeForgeError::InvalidRequest(format!("read error: {e}")))?;
        file_content.extend_from_slice(&data);
    }

    if file_content.is_empty() {
        guard.error("no file content received");
        return Err(edgeforge_core::EdgeForgeError::InvalidRequest("no file content".to_string()).into());
    }

    let file_size = file_content.len() as u64;
    let file_id = format!("file-{}", edgeforge_core::ModelId::new());
    let source_uri = filename
        .clone()
        .unwrap_or_else(|| format!("upload:{}", file_id));

    // Convert to text and chunk.
    let text = String::from_utf8_lossy(&file_content);
    let chunks = chunk_text(&text, DEFAULT_CHUNK_SIZE);
    let chunk_count = chunks.len() as u32;
    let embedder_model_id = state.embedder.model_id().to_string();

    tracing::info!(
        file_id = %file_id,
        source_uri = %source_uri,
        size = file_size,
        chunks = chunk_count,
        embedder_model_id = %embedder_model_id,
        "File uploaded and chunked — embedding and storing"
    );

    // Embed each chunk and store it in the vector DB.
    let mut stored: u32 = 0;
    for (idx, chunk_content) in chunks.iter().enumerate() {
        let vector = state.embedder.embed(chunk_content);
        let params = StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: source_uri.clone(),
            chunk_idx: Some(idx as u32),
            content: chunk_content.clone(),
            model_id: embedder_model_id.clone(),
            vector,
        };
        match state.embeddings.store_chunk(&params).await {
            Ok(source_id) => {
                stored += 1;
                tracing::debug!(
                    source_id = %source_id,
                    chunk_idx = idx,
                    "chunk stored"
                );
            }
            Err(e) => {
                tracing::warn!(
                    chunk_idx = idx,
                    error = %e,
                    "failed to store chunk (continuing with remaining chunks)"
                );
            }
        }
    }

    tracing::info!(
        file_id = %file_id,
        chunks = chunk_count,
        stored,
        "File ingestion complete"
    );

    let response = FileUploadResponse {
        id: file_id,
        status: "processed",
        chunks: chunk_count,
        stored,
        bytes: file_size,
        filename,
        model_id: embedder_model_id,
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
    fn test_chunk_text_zero_chunk_size() {
        let chunks = chunk_text("hello world", 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn test_chunk_text_long_word_no_spaces() {
        // A single very long word with no spaces — should not infinite loop.
        let text = "a".repeat(5000);
        let chunks = chunk_text(&text, 100);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 100);
        }
    }

    #[test]
    fn test_file_upload_response_serialize() {
        let resp = FileUploadResponse {
            id: "file-123".to_string(),
            status: "processed",
            chunks: 5,
            stored: 5,
            bytes: 1024,
            filename: Some("test.txt".to_string()),
            model_id: "edgeforge-hashing-768".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("file-123"));
        assert!(json.contains("processed"));
        assert!(json.contains("test.txt"));
        assert!(json.contains("edgeforge-hashing-768"));
    }
}
