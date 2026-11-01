-- 0016_crypto_inventory.sql
CREATE TABLE crypto_inventory (id TEXT PRIMARY KEY, location TEXT NOT NULL, primitive TEXT NOT NULL, variant TEXT NOT NULL, pqc_ready INTEGER NOT NULL, migration_phase TEXT NOT NULL, notes TEXT, last_audited_at INTEGER NOT NULL, git_sha TEXT NOT NULL);
CREATE TABLE crypto_handshake_latency (id TEXT PRIMARY KEY, kem_variant TEXT NOT NULL, sig_variant TEXT NOT NULL, handshake_ms INTEGER NOT NULL, measured_at INTEGER NOT NULL, peer_count INTEGER NOT NULL, notes TEXT);
ALTER TABLE benchmark_runs ADD COLUMN energy_joules_per_token REAL;
ALTER TABLE benchmark_runs ADD COLUMN carbon_grams_per_1k_tokens REAL;
ALTER TABLE benchmark_runs ADD COLUMN region TEXT;

