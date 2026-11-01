-- 0014_traceability.sql
CREATE TABLE traceability_items (item_path TEXT PRIMARY KEY, adr_ids TEXT NOT NULL, requirement_ids TEXT NOT NULL, verification_ids TEXT NOT NULL, cmmi_areas TEXT NOT NULL, last_updated_at INTEGER NOT NULL, git_sha TEXT NOT NULL);
CREATE TABLE adrs (id TEXT PRIMARY KEY, title TEXT NOT NULL, status TEXT NOT NULL, decided_at INTEGER NOT NULL, supersedes TEXT, superseded_by TEXT, file_path TEXT NOT NULL);

