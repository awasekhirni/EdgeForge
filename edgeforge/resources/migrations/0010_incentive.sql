-- 0010_incentive.sql
CREATE TABLE incentive_ledger (id TEXT PRIMARY KEY, peer_id TEXT NOT NULL REFERENCES pool_nodes(id) ON DELETE CASCADE, tier TEXT NOT NULL, kind TEXT NOT NULL CHECK (kind IN ('credit','debit')), amount INTEGER NOT NULL, reason TEXT NOT NULL, related_job TEXT REFERENCES jobs(id) ON DELETE SET NULL, created_at INTEGER NOT NULL);

