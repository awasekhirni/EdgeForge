-- 0012_prompt_audit.sql
CREATE TABLE prompt_guardrail_events (id TEXT PRIMARY KEY, job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL, stage TEXT NOT NULL, verdict TEXT NOT NULL, violation_category TEXT, detail TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE prompt_quarantine_events (id TEXT PRIMARY KEY, job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL, resource_uri TEXT NOT NULL, resource_sha256 BLOB NOT NULL, summary_text TEXT NOT NULL, confidence REAL NOT NULL, dropped INTEGER NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE prompt_pipeline_runs (id TEXT PRIMARY KEY, job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL, prompt_sha256 BLOB NOT NULL, planner_model TEXT NOT NULL, worker_model TEXT NOT NULL, quarantine_model TEXT, plan_json TEXT NOT NULL, action_json TEXT, outcome TEXT NOT NULL, created_at INTEGER NOT NULL, finished_at INTEGER);

