-- 0013_kpi_qppo.sql
CREATE TABLE kpi_measurements (id TEXT PRIMARY KEY, qppo_id TEXT NOT NULL, measured_value REAL NOT NULL, unit TEXT NOT NULL, source TEXT NOT NULL, measured_at INTEGER NOT NULL, git_sha TEXT);
CREATE TABLE kpi_violations (id TEXT PRIMARY KEY, qppo_id TEXT NOT NULL, measured_value REAL NOT NULL, status TEXT NOT NULL, draft_adr_id TEXT, resolved_at INTEGER, created_at INTEGER NOT NULL);

