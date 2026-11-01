//! # Embeddings ORM — RAG Vector Store (F-DEV-06)
//!
//! ## Purpose
//! Typed access to the `embedding_sources` + `embedding_vectors` tables.
//! Stores text chunks alongside their dense vector embeddings and provides
//! cosine-similarity search for RAG retrieval.
//!
//! ## What This File Does
//! - `EmbeddingSourceRow` — ORM row for `embedding_sources`
//! - `EmbeddingVectorRow` — ORM row for `embedding_vectors`
//! - `Embeddings` — typed access struct owning a `SqlitePool`
//! - `store_chunk(...)` — insert a chunk + its vector, returns the source ID
//! - `search(query_vec, model_id, top_k)` — cosine-similarity search across
//!   stored vectors for a given model, returns ranked results
//! - `list_by_model(model_id)` — list all sources for a model
//! - `list_by_source(source_uri)` — list all chunks from a file/URL
//! - `delete_by_source(source_uri)` — remove all chunks + vectors for a file
//! - `count(model_id)` — count stored vectors for a model
//!
//! ## Dependencies
//! - `sqlx::{SqlitePool, FromRow}` — SQLite ORM
//! - `edgeforge-core::{EdgeForgeError, ModelId}` — error type + IDs
//! - `edgeforge-runtime` — `cosine_similarity`, `decode_vector_f32le`,
//!   `encode_vector_f32le`, `l2_norm`
//! - `serde` — for row serialization
//! - `ulid::Ulid` — for generating unique source/vector IDs
//!
//! ## Output
//! `Result<String, EdgeForgeError>` from `store_chunk` (returns the source ID);
//! `Result<Vec<SearchResult>, EdgeForgeError>` from `search`.
//!
//! ## Function: store_chunk
//! - **Purpose:** Store a text chunk and its embedding vector
//! - **Parameters:** `&StoreChunkParams` — source_uri, chunk_idx, content,
//!   model_id, vector
//! - **Returns:** `Result<String, EdgeForgeError>` — the new source ID
//! - **Side effects:** Inserts a row into `embedding_sources` AND
//!   `embedding_vectors` (two INSERTs in a single transaction)
//! - **Errors:** Database error if the INSERT fails
//!
//! ## Function: search
//! - **Purpose:** Find the top-k most similar stored chunks to a query vector
//! - **Parameters:** `&[f32]` query vector, `model_id` filter, `top_k` limit
//! - **Returns:** `Result<Vec<SearchResult>, EdgeForgeError>` — ranked by
//!   cosine similarity, descending
//! - **Algorithm:**
//!   1. SELECT all vectors + sources for the given model_id
//!   2. Decode each vector BLOB into `Vec<f32>`
//!   3. Compute cosine similarity against the query vector
//!   4. Sort by similarity descending
//!   5. Take top_k
//! - **Performance:** O(n) where n = number of stored vectors for the model.
//!   Fine for small-to-medium corpora (thousands of chunks). For production
//!   scale, swap in sqlite-vss or a dedicated vector DB.

use crate::db::now_ms;
use edgeforge_core::{EdgeForgeError, ModelId};
use edgeforge_runtime::{cosine_similarity, decode_vector_f32le, encode_vector_f32le, l2_norm};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};
use ulid::Ulid;

/// ORM row for the `embedding_sources` table.
#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct EmbeddingSourceRow {
    /// Unique source ID (ULID).
    pub id: String,
    /// Source type: "file", "chunk", or "url".
    pub source_type: String,
    /// Source URI (file path, URL, etc.).
    pub source_uri: String,
    /// Chunk index within the source (NULL for non-chunked sources).
    pub chunk_idx: Option<i64>,
    /// The text content of this chunk.
    pub content: String,
    /// The embedding model ID used to embed this chunk.
    pub model_id: String,
    /// Unix timestamp (ms) when the chunk was embedded.
    pub embedded_at: i64,
}

/// ORM row for the `embedding_vectors` table.
#[derive(Clone, Debug, FromRow)]
pub struct EmbeddingVectorRow {
    /// Unique vector ID (ULID).
    pub id: String,
    /// FK to `embedding_sources.id`.
    pub source_id: String,
    /// The embedding model ID.
    pub model_id: String,
    /// Vector dimensionality.
    pub dim: i64,
    /// The vector bytes (little-endian f32).
    pub vector: Vec<u8>,
    /// Precomputed L2 norm.
    pub norm: f64,
    /// Unix timestamp (ms).
    pub created_at: i64,
}

/// Parameters for [`Embeddings::store_chunk`].
#[derive(Clone, Debug)]
pub struct StoreChunkParams {
    /// Source type: "file", "chunk", or "url".
    pub source_type: String,
    /// Source URI (file path, URL, etc.).
    pub source_uri: String,
    /// Chunk index within the source (None for non-chunked).
    pub chunk_idx: Option<u32>,
    /// The text content of this chunk.
    pub content: String,
    /// The embedding model ID.
    pub model_id: String,
    /// The embedding vector (will be encoded as LE f32 bytes).
    pub vector: Vec<f32>,
}

/// A single search result from [`Embeddings::search`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// The source ID.
    pub source_id: String,
    /// The source URI (file path, URL, etc.).
    pub source_uri: String,
    /// Chunk index within the source.
    pub chunk_idx: Option<i64>,
    /// The text content of the matched chunk.
    pub content: String,
    /// Cosine similarity score in [-1, 1].
    pub score: f32,
    /// The embedding model ID.
    pub model_id: String,
}

/// Typed access to the `embedding_sources` + `embedding_vectors` tables.
pub struct Embeddings {
    pool: SqlitePool,
}

impl Embeddings {
    /// Create a new `Embeddings` ORM backed by the given SQLite pool.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a text chunk and its embedding vector.
    ///
    /// Inserts a row into both `embedding_sources` and `embedding_vectors`
    /// within a single transaction. Returns the generated source ID.
    ///
    /// ## Parameters
    /// - `params`: The chunk metadata + vector (see [`StoreChunkParams`]).
    ///
    /// ## Returns
    /// `Result<String, EdgeForgeError>` — the new source ID (ULID string).
    ///
    /// ## Errors
    /// - `Database` if either INSERT fails.
    pub async fn store_chunk(&self, params: &StoreChunkParams) -> Result<String, EdgeForgeError> {
        let source_id = Ulid::new().to_string();
        let vector_id = Ulid::new().to_string();
        let now = now_ms();
        let dim = params.vector.len() as i64;
        let vector_bytes = encode_vector_f32le(&params.vector);
        let norm = f64::from(l2_norm(&params.vector));

        tracing::info!(
            source_id = %source_id,
            source_uri = %params.source_uri,
            chunk_idx = ?params.chunk_idx,
            model_id = %params.model_id,
            dim,
            "Embeddings: storing chunk"
        );

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(crate::db_err)?;

        sqlx::query(
            "INSERT INTO embedding_sources (id, source_type, source_uri, chunk_idx, content, model_id, embedded_at) VALUES (?,?,?,?,?,?,?)",
        )
        .bind(&source_id)
        .bind(&params.source_type)
        .bind(&params.source_uri)
        .bind(params.chunk_idx.map(|v| v as i64))
        .bind(&params.content)
        .bind(&params.model_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(crate::db_err)?;

        sqlx::query(
            "INSERT INTO embedding_vectors (id, source_id, model_id, dim, vector, norm, created_at) VALUES (?,?,?,?,?,?,?)",
        )
        .bind(&vector_id)
        .bind(&source_id)
        .bind(&params.model_id)
        .bind(dim)
        .bind(&vector_bytes)
        .bind(norm)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(crate::db_err)?;

        tx.commit().await.map_err(crate::db_err)?;

        tracing::info!(source_id = %source_id, "Embeddings: chunk stored");

        Ok(source_id)
    }

    /// Search for the top-k most similar chunks to a query vector.
    ///
    /// ## Parameters
    /// - `query_vec`: The query embedding vector.
    /// - `model_id`: Filter to only vectors from this embedding model.
    /// - `top_k`: Maximum number of results to return.
    ///
    /// ## Returns
    /// `Result<Vec<SearchResult>, EdgeForgeError>` — ranked by cosine
    /// similarity, descending.
    ///
    /// ## Algorithm
    /// 1. SELECT all vectors + sources for the given `model_id`.
    /// 2. Decode each vector BLOB into `Vec<f32>`.
    /// 3. Compute cosine similarity against `query_vec`.
    /// 4. Sort by similarity descending.
    /// 5. Take `top_k`.
    ///
    /// ## Errors
    /// - `Database` if the SELECT fails.
    pub async fn search(
        &self,
        query_vec: &[f32],
        model_id: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, EdgeForgeError> {
        tracing::info!(
            model_id = %model_id,
            top_k,
            query_dim = query_vec.len(),
            "Embeddings: searching"
        );

        // Join embedding_vectors with embedding_sources on source_id.
        let rows: Vec<(EmbeddingVectorRow, EmbeddingSourceRow)> = sqlx::query_as(
            "SELECT v.id as \"v_id\", v.source_id as \"v_source_id\", v.model_id as \"v_model_id\", v.dim as \"v_dim\", v.vector as \"v_vector\", v.norm as \"v_norm\", v.created_at as \"v_created_at\", \
                    s.id as \"s_id\", s.source_type as \"s_source_type\", s.source_uri as \"s_source_uri\", s.chunk_idx as \"s_chunk_idx\", s.content as \"s_content\", s.model_id as \"s_model_id\", s.embedded_at as \"s_embedded_at\" \
             FROM embedding_vectors v \
             JOIN embedding_sources s ON v.source_id = s.id \
             WHERE v.model_id = ?",
        )
        .bind(model_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Embeddings: search query failed");
            crate::db_err(e)
        })?;

        tracing::debug!(candidate_count = rows.len(), "Embeddings: candidates loaded");

        let mut results: Vec<SearchResult> = rows
            .into_iter()
            .filter_map(|(vec_row, src_row)| {
                let stored_vec = decode_vector_f32le(&vec_row.vector);
                if stored_vec.is_empty() || stored_vec.len() != query_vec.len() {
                    tracing::warn!(
                        source_id = %src_row.id,
                        stored_dim = stored_vec.len(),
                        query_dim = query_vec.len(),
                        "Embeddings: dimension mismatch, skipping"
                    );
                    return None;
                }
                let score = cosine_similarity(query_vec, &stored_vec);
                Some(SearchResult {
                    source_id: src_row.id,
                    source_uri: src_row.source_uri,
                    chunk_idx: src_row.chunk_idx,
                    content: src_row.content,
                    score,
                    model_id: src_row.model_id,
                })
            })
            .collect();

        // Sort by score descending.
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top_k.
        results.truncate(top_k);

        tracing::info!(
            result_count = results.len(),
            top_score = results.first().map(|r| r.score).unwrap_or(0.0),
            "Embeddings: search complete"
        );

        Ok(results)
    }

    /// List all embedding sources for a given model.
    ///
    /// ## Parameters
    /// - `model_id`: The embedding model ID to filter by.
    ///
    /// ## Returns
    /// `Result<Vec<EmbeddingSourceRow>, EdgeForgeError>`.
    pub async fn list_by_model(
        &self,
        model_id: &str,
    ) -> Result<Vec<EmbeddingSourceRow>, EdgeForgeError> {
        Ok(sqlx::query_as::<_, EmbeddingSourceRow>(
            "SELECT * FROM embedding_sources WHERE model_id = ? ORDER BY embedded_at DESC",
        )
        .bind(model_id)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?)
    }

    /// List all embedding sources from a given file/URL.
    ///
    /// ## Parameters
    /// - `source_uri`: The source URI to filter by.
    ///
    /// ## Returns
    /// `Result<Vec<EmbeddingSourceRow>, EdgeForgeError>` — ordered by chunk_idx ASC.
    pub async fn list_by_source(
        &self,
        source_uri: &str,
    ) -> Result<Vec<EmbeddingSourceRow>, EdgeForgeError> {
        Ok(sqlx::query_as::<_, EmbeddingSourceRow>(
            "SELECT * FROM embedding_sources WHERE source_uri = ? ORDER BY chunk_idx ASC",
        )
        .bind(source_uri)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?)
    }

    /// Delete all chunks + vectors for a given source URI.
    ///
    /// ## Parameters
    /// - `source_uri`: The source URI whose chunks should be deleted.
    ///
    /// ## Returns
    /// `Result<u64, EdgeForgeError>` — the number of source rows deleted.
    ///
    /// ## Side Effects
    /// Deletes from both `embedding_sources` and `embedding_vectors`. The
    /// `embedding_vectors` rows are deleted first (by source_id), then the
    /// `embedding_sources` rows.
    pub async fn delete_by_source(
        &self,
        source_uri: &str,
    ) -> Result<u64, EdgeForgeError> {
        let mut tx = self.pool.begin().await.map_err(crate::db_err)?;

        // First, collect the source IDs to delete.
        let source_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM embedding_sources WHERE source_uri = ?",
        )
        .bind(source_uri)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db_err)?;

        if source_ids.is_empty() {
            tx.commit().await.map_err(crate::db_err)?;
            return Ok(0);
        }

        // Delete vectors for those source IDs.
        for sid in &source_ids {
            sqlx::query("DELETE FROM embedding_vectors WHERE source_id = ?")
                .bind(sid)
                .execute(&mut *tx)
                .await
                .map_err(crate::db_err)?;
        }

        // Delete the sources themselves.
        let deleted = sqlx::query("DELETE FROM embedding_sources WHERE source_uri = ?")
            .bind(source_uri)
            .execute(&mut *tx)
            .await
            .map_err(crate::db_err)?;

        let rows = deleted.rows_affected();

        tx.commit().await.map_err(crate::db_err)?;

        tracing::info!(
            source_uri = %source_uri,
            deleted_count = rows,
            "Embeddings: deleted chunks for source"
        );

        Ok(rows)
    }

    /// Count the number of stored vectors for a given model.
    ///
    /// ## Parameters
    /// - `model_id`: The embedding model ID.
    ///
    /// ## Returns
    /// `Result<i64, EdgeForgeError>` — the count.
    pub async fn count(&self, model_id: &str) -> Result<i64, EdgeForgeError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM embedding_vectors WHERE model_id = ?")
            .bind(model_id)
            .fetch_one(&self.pool)
            .await
            .map_err(crate::db_err)?)
    }

    /// Convert a `ModelId` to the string form used in the `model_id` column.
    /// (Convenience — the `model_id` column stores plain strings, not ULIDs.)
    #[must_use]
    pub fn model_id_str(id: ModelId) -> String {
        id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_runtime::{HashingEmbedder, EmbeddingEngine};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> Embeddings {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await.unwrap();
        sqlx::query("CREATE TABLE embedding_sources (id TEXT PRIMARY KEY, source_type TEXT NOT NULL, source_uri TEXT NOT NULL, chunk_idx INTEGER, content TEXT NOT NULL, model_id TEXT NOT NULL, embedded_at INTEGER NOT NULL)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE embedding_vectors (id TEXT PRIMARY KEY, source_id TEXT NOT NULL, model_id TEXT NOT NULL, dim INTEGER NOT NULL, vector BLOB NOT NULL, norm REAL NOT NULL, created_at INTEGER NOT NULL)")
            .execute(&pool).await.unwrap();
        Embeddings::new(pool)
    }

    #[tokio::test]
    async fn test_store_and_search() {
        let orm = setup().await;
        let embedder = HashingEmbedder::new();
        let model_id = embedder.model_id().to_string();

        let vec = embedder.embed("hello world");
        let id = orm.store_chunk(&StoreChunkParams {
            source_type: "file".into(), source_uri: "doc.txt".into(),
            chunk_idx: Some(0), content: "hello world".into(),
            model_id: model_id.clone(), vector: vec,
        }).await.unwrap();
        assert!(!id.is_empty());

        let qvec = embedder.embed("hello");
        let results = orm.search(&qvec, &model_id, 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("hello"));
        assert!(results[0].score > 0.0);
    }

    #[tokio::test]
    async fn test_list_by_model() {
        let orm = setup().await;
        let embedder = HashingEmbedder::new();
        let mid = embedder.model_id().to_string();
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".into(), source_uri: "d.txt".into(),
            chunk_idx: Some(0), content: "test".into(),
            model_id: mid.clone(), vector: embedder.embed("test"),
        }).await.unwrap();
        let rows = orm.list_by_model(&mid).await.unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn test_count_empty() {
        let orm = setup().await;
        assert_eq!(orm.count("any-model").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_delete_by_source() {
        let orm = setup().await;
        let embedder = HashingEmbedder::new();
        let mid = embedder.model_id().to_string();
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".into(), source_uri: "del.txt".into(),
            chunk_idx: Some(0), content: "del".into(),
            model_id: mid, vector: embedder.embed("del"),
        }).await.unwrap();
        let deleted = orm.delete_by_source("del.txt").await.unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(orm.count("any").await.unwrap(), 0);
    }
}
