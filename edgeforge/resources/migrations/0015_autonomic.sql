-- 0015_autonomic.sql
CREATE TABLE autonomic_events (id TEXT PRIMARY KEY, event_type TEXT NOT NULL, event_json TEXT NOT NULL, ingested_at INTEGER NOT NULL);
CREATE TABLE autonomic_patterns (id TEXT PRIMARY KEY, failure_kind TEXT NOT NULL, metric TEXT NOT NULL, value REAL NOT NULL, target TEXT, variation_kind TEXT NOT NULL, occurrences INTEGER NOT NULL DEFAULT 1, first_seen_ms INTEGER NOT NULL, last_seen_ms INTEGER NOT NULL);
CREATE TABLE autonomic_actions (id TEXT PRIMARY KEY, remediation_kind TEXT NOT NULL, target TEXT NOT NULL, result_kind TEXT NOT NULL, result_detail TEXT NOT NULL, pattern_id TEXT, generated_test_path TEXT, generated_test_kind TEXT, created_at INTEGER NOT NULL);

