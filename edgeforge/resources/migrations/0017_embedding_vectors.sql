-- 0017_embedding_vectors.sql
--
-- Stores embedding vectors for RAG (F-DEV-06). Each row links a text chunk
-- (already stored in embedding_sources) to its dense vector representation,
-- encoded as little-endian f32 bytes. The L2 norm is precomputed at insert
-- time so cosine similarity search is a single dot-product + two divisions.
--
-- This table is intentionally loose (no FK to models) so embedding models
-- that are not in the models registry (e.g. the built-in HashingEmbedder)
-- can still be used.

CREATE TABLE IF NOT EXISTS embedding_vectors (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    dim         INTEGER NOT NULL CHECK (dim > 0),
    vector      BLOB NOT NULL,
    norm        REAL NOT NULL CHECK (norm >= 0),
    created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_embedding_vectors_model  ON embedding_vectors(model_id);
CREATE INDEX IF NOT EXISTS idx_embedding_vectors_source ON embedding_vectors(source_id);
