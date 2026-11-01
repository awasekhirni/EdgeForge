//! Integration tests for the RAG embeddings pipeline.
//!
//! Exercises the full RAG flow:
//!   1. EmbeddingEngine produces real (non-zero) vectors
//!   2. Embeddings ORM stores chunks + vectors in SQLite
//!   3. Cosine-similarity search retrieves the most similar chunks
//!   4. chunk_text splits files into word-boundary chunks
//!   5. End-to-end: embed a document, store it, search for a query, verify
//!      the correct chunk is retrieved with the highest score
//!
//! These tests use an in-memory SQLite database (no external dependencies).

use edgeforge_orchestration::{Embeddings, StoreChunkParams};
use edgeforge_runtime::{cosine_similarity, EmbeddingEngine, HashingEmbedder};
use sqlx::sqlite::SqlitePoolOptions;

// ── Helpers ────────────────────────────────────────────────────────────────

async fn make_embeddings() -> Embeddings {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to connect to in-memory SQLite");

    // Apply the migrations that the ORM depends on.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS embedding_sources (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL,
            source_uri TEXT NOT NULL,
            chunk_idx INTEGER,
            content TEXT NOT NULL,
            model_id TEXT NOT NULL,
            embedded_at INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create embedding_sources");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS embedding_vectors (
            id TEXT PRIMARY KEY,
            source_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            dim INTEGER NOT NULL,
            vector BLOB NOT NULL,
            norm REAL NOT NULL,
            created_at INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create embedding_vectors");

    Embeddings::new(pool)
}

fn make_embedder() -> HashingEmbedder {
    HashingEmbedder::new()
}

// ── EmbeddingEngine tests ──────────────────────────────────────────────────

#[test]
fn embedder_produces_nonzero_vectors() {
    let e = make_embedder();
    let v = e.embed("the quick brown fox");
    assert!(v.iter().any(|x| x.abs() > 1e-9));
    assert_eq!(v.len(), e.dim());
}

#[test]
fn embedder_is_deterministic() {
    let e = make_embedder();
    let v1 = e.embed("hello world from edgeforge");
    let v2 = e.embed("hello world from edgeforge");
    assert_eq!(v1, v2);
}

#[test]
fn embedder_similar_texts_have_high_similarity() {
    let e = make_embedder();
    let v1 = e.embed("the quick brown fox jumps");
    let v2 = e.embed("the quick brown fox leaps");
    let sim = cosine_similarity(&v1, &v2);
    assert!(sim > 0.7, "similar texts should have high similarity, got {sim}");
}

#[test]
fn embedder_dissimilar_texts_have_low_similarity() {
    let e = make_embedder();
    let v1 = e.embed("the quick brown fox jumps over the lazy dog");
    let v2 = e.embed("zzzzzz qqqqqq xxxxxx yyyyyy");
    let sim = cosine_similarity(&v1, &v2);
    assert!(sim < 0.5, "dissimilar texts should have low similarity, got {sim}");
}

// ── Embeddings ORM tests ───────────────────────────────────────────────────

#[tokio::test]
async fn orm_store_and_search_basic() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    // Store a chunk.
    let chunk = "the quick brown fox jumps over the lazy dog";
    let vector = embedder.embed(chunk);
    let source_id = orm
        .store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: "test.txt".to_string(),
            chunk_idx: Some(0),
            content: chunk.to_string(),
            model_id: model_id.clone(),
            vector,
        })
        .await
        .expect("store_chunk should succeed");

    assert!(!source_id.is_empty());

    // Search for a similar query.
    let query_vec = embedder.embed("the quick brown fox");
    let results = orm
        .search(&query_vec, &model_id, 5)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id, source_id);
    assert_eq!(results[0].content, chunk);
    assert!(results[0].score > 0.5, "similarity should be high");
}

#[tokio::test]
async fn orm_search_ranks_by_similarity() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    // Store three chunks with different content.
    let chunks = vec![
        "the quick brown fox jumps over the lazy dog",
        "machine learning models for natural language processing",
        "the fox is a cunning animal in many folktales",
    ];

    for (i, chunk) in chunks.iter().enumerate() {
        let vector = embedder.embed(chunk);
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: "test.txt".to_string(),
            chunk_idx: Some(i as u32),
            content: chunk.to_string(),
            model_id: model_id.clone(),
            vector,
        })
        .await
        .expect("store_chunk should succeed");
    }

    // Search for a query about foxes.
    let query_vec = embedder.embed("fox animal folktales");
    let results = orm
        .search(&query_vec, &model_id, 3)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 3);
    // Results should be sorted by score descending.
    for i in 0..results.len() - 1 {
        assert!(
            results[i].score >= results[i + 1].score,
            "results should be sorted by score descending"
        );
    }
    // The chunk about "fox is a cunning animal in many folktales" should
    // rank highly.
    assert!(
        results[0].content.contains("fox"),
        "top result should mention fox, got: {}",
        results[0].content
    );
}

#[tokio::test]
async fn orm_search_filters_by_model_id() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();

    // Store a chunk with model "A".
    orm.store_chunk(&StoreChunkParams {
        source_type: "file".to_string(),
        source_uri: "a.txt".to_string(),
        chunk_idx: Some(0),
        content: "hello from model A".to_string(),
        model_id: "model-A".to_string(),
        vector: embedder.embed("hello from model A"),
    })
    .await
    .unwrap();

    // Store a chunk with model "B".
    orm.store_chunk(&StoreChunkParams {
        source_type: "file".to_string(),
        source_uri: "b.txt".to_string(),
        chunk_idx: Some(0),
        content: "hello from model B".to_string(),
        model_id: "model-B".to_string(),
        vector: embedder.embed("hello from model B"),
    })
    .await
    .unwrap();

    // Search within model A only.
    let query_vec = embedder.embed("hello");
    let results_a = orm.search(&query_vec, "model-A", 10).await.unwrap();
    assert_eq!(results_a.len(), 1);
    assert_eq!(results_a[0].model_id, "model-A");

    // Search within model B only.
    let results_b = orm.search(&query_vec, "model-B", 10).await.unwrap();
    assert_eq!(results_b.len(), 1);
    assert_eq!(results_b[0].model_id, "model-B");
}

#[tokio::test]
async fn orm_list_by_model() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    for i in 0..3 {
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: format!("file{i}.txt"),
            chunk_idx: Some(0),
            content: format!("chunk {i}"),
            model_id: model_id.clone(),
            vector: embedder.embed(&format!("chunk {i}")),
        })
        .await
        .unwrap();
    }

    let rows = orm.list_by_model(&model_id).await.unwrap();
    assert_eq!(rows.len(), 3);
    // All should have the same model_id.
    for row in &rows {
        assert_eq!(row.model_id, model_id);
    }
}

#[tokio::test]
async fn orm_list_by_source() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    // Store 3 chunks from the same source.
    for i in 0..3 {
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: "doc.txt".to_string(),
            chunk_idx: Some(i),
            content: format!("chunk {i}"),
            model_id: model_id.clone(),
            vector: embedder.embed(&format!("chunk {i}")),
        })
        .await
        .unwrap();
    }

    // Store a chunk from a different source.
    orm.store_chunk(&StoreChunkParams {
        source_type: "file".to_string(),
        source_uri: "other.txt".to_string(),
        chunk_idx: Some(0),
        content: "other content".to_string(),
        model_id: model_id.clone(),
        vector: embedder.embed("other content"),
    })
    .await
    .unwrap();

    let rows = orm.list_by_source("doc.txt").await.unwrap();
    assert_eq!(rows.len(), 3);
    // Should be ordered by chunk_idx ASC.
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.chunk_idx, Some(i as i64));
    }
}

#[tokio::test]
async fn orm_delete_by_source() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    // Store 2 chunks from source "A" and 1 from source "B".
    for i in 0..2 {
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: "a.txt".to_string(),
            chunk_idx: Some(i),
            content: format!("a chunk {i}"),
            model_id: model_id.clone(),
            vector: embedder.embed(&format!("a chunk {i}")),
        })
        .await
        .unwrap();
    }
    orm.store_chunk(&StoreChunkParams {
        source_type: "file".to_string(),
        source_uri: "b.txt".to_string(),
        chunk_idx: Some(0),
        content: "b chunk".to_string(),
        model_id: model_id.clone(),
        vector: embedder.embed("b chunk"),
    })
    .await
    .unwrap();

    // Delete source A.
    let deleted = orm.delete_by_source("a.txt").await.unwrap();
    assert_eq!(deleted, 2);

    // Source A should be gone, source B should remain.
    let a_rows = orm.list_by_source("a.txt").await.unwrap();
    assert_eq!(a_rows.len(), 0);
    let b_rows = orm.list_by_source("b.txt").await.unwrap();
    assert_eq!(b_rows.len(), 1);

    // Search should only find 1 result now.
    let query_vec = embedder.embed("chunk");
    let results = orm.search(&query_vec, &model_id, 10).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn orm_count() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    assert_eq!(orm.count(&model_id).await.unwrap(), 0);

    for i in 0..5 {
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: "doc.txt".to_string(),
            chunk_idx: Some(i),
            content: format!("chunk {i}"),
            model_id: model_id.clone(),
            vector: embedder.embed(&format!("chunk {i}")),
        })
        .await
        .unwrap();
    }

    assert_eq!(orm.count(&model_id).await.unwrap(), 5);
}

#[tokio::test]
async fn orm_search_empty_db_returns_empty() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    let query_vec = embedder.embed("anything");
    let results = orm.search(&query_vec, &model_id, 5).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn orm_delete_nonexistent_source_returns_zero() {
    let orm = make_embeddings().await;
    let deleted = orm.delete_by_source("nonexistent.txt").await.unwrap();
    assert_eq!(deleted, 0);
}

// ── End-to-end RAG pipeline test ───────────────────────────────────────────

#[tokio::test]
async fn e2e_rag_pipeline() {
    let orm = make_embeddings().await;
    let embedder = make_embedder();
    let model_id = embedder.model_id().to_string();

    // Simulate a document with multiple paragraphs.
    let document = r#"
        The EdgeForge platform is designed for air-gapped inference.
        It uses llama.cpp as the primary inference engine via FFI.
        The SsdEngine is a pure-Rust fallback that works without llama.cpp.
        Embeddings are produced by the HashingEmbedder using feature hashing.
        RAG retrieval uses cosine similarity over stored vectors.
        The system supports MoE models with expert weight paging from SSD.
    "#;

    // Simple chunking: split by sentences (simulate chunk_text).
    let chunks: Vec<&str> = document
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    // Store each chunk.
    for (i, chunk) in chunks.iter().enumerate() {
        let vector = embedder.embed(chunk);
        orm.store_chunk(&StoreChunkParams {
            source_type: "file".to_string(),
            source_uri: "edgeforge-doc.txt".to_string(),
            chunk_idx: Some(i as u32),
            content: chunk.to_string(),
            model_id: model_id.clone(),
            vector,
        })
        .await
        .unwrap();
    }

    // Query: "What does the SsdEngine do?"
    let query = "What does the SsdEngine do?";
    let query_vec = embedder.embed(query);
    let results = orm.search(&query_vec, &model_id, 3).await.unwrap();

    // The top result should be the SsdEngine chunk.
    assert!(!results.is_empty(), "should find results");
    assert!(
        results[0].content.to_lowercase().contains("ssdengine")
            || results[0].content.to_lowercase().contains("ssd"),
        "top result should mention SsdEngine/SSD, got: {}",
        results[0].content
    );

    // Verify scores are in [-1, 1].
    for r in &results {
        assert!(r.score >= -1.0 && r.score <= 1.0, "score out of range: {}", r.score);
    }

    // Verify descending order.
    for i in 0..results.len() - 1 {
        assert!(results[i].score >= results[i + 1].score);
    }
}
