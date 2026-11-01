-- 0002_models.sql
CREATE TABLE models (id TEXT PRIMARY KEY, name TEXT NOT NULL, publisher TEXT NOT NULL, license TEXT NOT NULL, file_path TEXT NOT NULL, file_size_bytes INTEGER NOT NULL CHECK (file_size_bytes > 0), sha256 BLOB NOT NULL, precision TEXT NOT NULL CHECK (precision IN ('fp32','fp16','bf16','q8','q4','q3')), arch TEXT NOT NULL CHECK (arch IN ('moe','dense','rwkv','mamba')), total_params INTEGER NOT NULL, active_params INTEGER, dense_core_bytes INTEGER NOT NULL, expert_pool_bytes INTEGER NOT NULL, imported_at INTEGER NOT NULL, last_used_at INTEGER);
CREATE INDEX idx_models_name ON models(name);
CREATE INDEX idx_models_last_used ON models(last_used_at);

