-- 0018_relax_embedding_sources_fk.sql
--
-- Drops the FOREIGN KEY constraint on embedding_sources.model_id so that
-- embedding models (like the built-in HashingEmbedder) that are not in the
-- models registry can still store vectors. The original FK was overly
-- restrictive — embedding models are conceptually different from generation
-- models and may not have GGUF file paths, SHA-256 hashes, etc.
--
-- SQLite does not support ALTER TABLE DROP CONSTRAINT, so we use the
-- standard "12-step" table recreation pattern:
--   1. Create the new table without the FK
--   2. Copy all rows
--   3. Drop the old table
--   4. Rename the new table to the original name
--   5. Recreate the index

CREATE TABLE embedding_sources_new (
    id          TEXT PRIMARY KEY,
    source_type TEXT NOT NULL CHECK (source_type IN ('file','chunk','url')),
    source_uri  TEXT NOT NULL,
    chunk_idx   INTEGER,
    content     TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    embedded_at INTEGER NOT NULL
);

INSERT INTO embedding_sources_new
    SELECT id, source_type, source_uri, chunk_idx, content, model_id, embedded_at
    FROM embedding_sources;

DROP TABLE embedding_sources;

ALTER TABLE embedding_sources_new RENAME TO embedding_sources;

CREATE INDEX IF NOT EXISTS idx_embedding_sources_model ON embedding_sources(model_id);
