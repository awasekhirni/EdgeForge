-- 0007_embeddings.sql
CREATE TABLE IF NOT EXISTS embedding_sources (id TEXT PRIMARY KEY, source_type TEXT NOT NULL CHECK (source_type IN ('file','chunk','url')), source_uri TEXT NOT NULL, chunk_idx INTEGER, content TEXT NOT NULL, model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE, embedded_at INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS idx_embedding_sources_model ON embedding_sources(model_id);

