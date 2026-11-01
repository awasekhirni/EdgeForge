-- 0006_benchmarks.sql
CREATE TABLE benchmark_runs (id TEXT PRIMARY KEY, model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE, preset TEXT NOT NULL, precision TEXT NOT NULL, warmup_tokens INTEGER NOT NULL, measured_tokens INTEGER NOT NULL, tokens_per_sec REAL NOT NULL, first_token_ms INTEGER NOT NULL, ssd_read_mbps REAL, ssd_temp_c REAL, cold_cache INTEGER NOT NULL, ran_at INTEGER NOT NULL);

