-- 0003_jobs.sql
CREATE TABLE jobs (id TEXT PRIMARY KEY, model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE, prompt_hash BLOB NOT NULL, status TEXT NOT NULL CHECK (status IN ('queued','running','paused','done','failed','cancelled')), preset TEXT NOT NULL, priority TEXT NOT NULL CHECK (priority IN ('interactive','batch')), params_json TEXT NOT NULL, tokens_generated INTEGER NOT NULL DEFAULT 0, checkpoint_path TEXT, error_msg TEXT, created_at INTEGER NOT NULL, started_at INTEGER, finished_at INTEGER);
CREATE INDEX idx_jobs_status ON jobs(status, created_at);

