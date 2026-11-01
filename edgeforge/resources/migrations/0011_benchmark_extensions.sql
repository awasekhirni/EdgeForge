-- 0011_benchmark_extensions.sql
ALTER TABLE benchmark_runs ADD COLUMN speculation_acceptance_rate REAL;
ALTER TABLE benchmark_runs ADD COLUMN prefetch_hit_rate REAL;
ALTER TABLE benchmark_runs ADD COLUMN surrogate_accuracy_delta REAL;
ALTER TABLE benchmark_runs ADD COLUMN crowd_tier TEXT;
ALTER TABLE benchmark_runs ADD COLUMN verification_pending_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE benchmark_runs ADD COLUMN verification_corrected_count INTEGER NOT NULL DEFAULT 0;

