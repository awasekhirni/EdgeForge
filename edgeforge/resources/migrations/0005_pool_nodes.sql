-- 0005_pool_nodes.sql
CREATE TABLE pool_nodes (id TEXT PRIMARY KEY, hostname TEXT NOT NULL, addr TEXT NOT NULL, cert_pem TEXT NOT NULL, policy TEXT NOT NULL CHECK (policy IN ('self_only','trusted_peers','lan_open')), bandwidth_cap_bps INTEGER, last_seen_at INTEGER NOT NULL, trusted INTEGER NOT NULL DEFAULT 0);

