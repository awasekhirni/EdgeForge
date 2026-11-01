#!/usr/bin/env python3
"""
validate_migrations.py — Apply all 16 EdgeForge migrations to a fresh in-memory
SQLite database using Python's built-in sqlite3 module.

This simulates what sqlx::migrate!() does at runtime:
  1. Creates the _sqlx_migrations table.
  2. Runs each .sql file in numeric order, recording each as applied.
  3. Verifies that all expected tables exist after the run.
  4. Performs a sample INSERT into `models` and `meta` to prove the schema
     is real (the original bug: empty schema → INSERT failure).

Exit code 0 = all migrations applied cleanly + tables exist + sample inserts work.
Exit code 1 = any failure (printed to stderr).
"""

from __future__ import annotations

import sqlite3
import sys
from pathlib import Path


EXPECTED_TABLES = {
    "meta",
    "models",
    "jobs",
    "conversations",
    "messages",
    "pool_nodes",
    "benchmark_runs",
    "embedding_sources",
    "peer_reputation",
    "verification_events",
    "incentive_ledger",
    "prompt_guardrail_events",
    "prompt_quarantine_events",
    "prompt_pipeline_runs",
    "kpi_measurements",
    "kpi_violations",
    "traceability_items",
    "adrs",
    "autonomic_events",
    "autonomic_patterns",
    "autonomic_actions",
    "crypto_inventory",
    "crypto_handshake_latency",
    "embedding_vectors",
    # embedding_sources is recreated by 0018 (FK relaxed) — still in the set
}


def apply_migrations(migrations_dir: Path) -> None:
    sql_files = sorted(migrations_dir.glob("*.sql"))
    if not sql_files:
        raise RuntimeError(f"no .sql files in {migrations_dir}")

    conn = sqlite3.connect(":memory:")
    conn.execute("PRAGMA foreign_keys = ON")
    cur = conn.cursor()

    # Replicate sqlx's migrations bookkeeping table.
    cur.execute(
        """
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            success BOOLEAN NOT NULL,
            checksum BLOB NOT NULL,
            execution_time BIGINT NOT NULL
        )
        """
    )

    print(f"Applying {len(sql_files)} migrations to in-memory SQLite ...")
    for f in sql_files:
        # version = numeric prefix
        stem = f.stem  # e.g. "0001_init"
        try:
            version = int(stem.split("_")[0])
        except ValueError as e:
            raise RuntimeError(f"bad migration filename {f.name}: {e}") from e
        description = stem.split("_", 1)[1] if "_" in stem else stem

        sql = f.read_text(encoding="utf-8")
        if "\\n" in sql:
            raise RuntimeError(
                f"{f.name}: literal '\\n' still present in file (fix_migrations.py did not run)"
            )

        cur.execute("BEGIN")
        try:
            cur.executescript(sql)
            cur.execute(
                "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) "
                "VALUES (?, ?, 1, zeroblob(32), 0)",
                (version, description),
            )
            cur.execute("COMMIT")
            print(f"  applied {f.name} (version={version})")
        except sqlite3.Error as e:
            cur.execute("ROLLBACK")
            raise RuntimeError(f"{f.name} failed: {e}") from e

    # Verify tables exist.
    cur.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
    actual_tables = {row[0] for row in cur.fetchall()}

    missing = EXPECTED_TABLES - actual_tables
    if missing:
        raise RuntimeError(f"missing tables after migration: {sorted(missing)}")

    extra = actual_tables - EXPECTED_TABLES - {"_sqlx_migrations"}
    if extra:
        print(f"  (note: extra tables present: {sorted(extra)})")

    # Sample INSERT into meta (originally failed: empty schema).
    cur.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', '1') "
        "ON CONFLICT(key) DO NOTHING"
    )

    # Sample INSERT into models (originally failed: empty schema).
    cur.execute(
        "INSERT INTO models (id, name, publisher, license, file_path, file_size_bytes, "
        "sha256, precision, arch, total_params, dense_core_bytes, expert_pool_bytes, imported_at) "
        "VALUES ('01J9TEST', 'Test Model', 'test', 'Apache-2.0', '/tmp/model.gguf', "
        "1024, zeroblob(32), 'fp16', 'moe', 1000000000, 100000000, 900000000, 1700000000000)"
    )

    # Sample INSERT into jobs (FK to models).
    cur.execute(
        "INSERT INTO jobs (id, model_id, prompt_hash, status, preset, priority, params_json, "
        "created_at) VALUES ('job1', '01J9TEST', zeroblob(32), 'queued', 'balanced', 'interactive', '{}', 1700000000000)"
    )

    # Test ALTER TABLE columns from 0011 and 0016.
    cur.execute("PRAGMA table_info(benchmark_runs)")
    cols = {row[1] for row in cur.fetchall()}
    required_alter_cols = {
        "speculation_acceptance_rate",
        "prefetch_hit_rate",
        "surrogate_accuracy_delta",
        "crowd_tier",
        "verification_pending_count",
        "verification_corrected_count",
        "energy_joules_per_token",
        "carbon_grams_per_1k_tokens",
        "region",
    }
    missing_cols = required_alter_cols - cols
    if missing_cols:
        raise RuntimeError(f"ALTER TABLE columns missing from benchmark_runs: {sorted(missing_cols)}")

    # Sample benchmark insert exercising the ALTER columns.
    cur.execute(
        "INSERT INTO benchmark_runs (id, model_id, preset, precision, warmup_tokens, measured_tokens, "
        "tokens_per_sec, first_token_ms, ssd_read_mbps, ssd_temp_c, cold_cache, ran_at, "
        "speculation_acceptance_rate, prefetch_hit_rate, surrogate_accuracy_delta, crowd_tier, "
        "verification_pending_count, verification_corrected_count, energy_joules_per_token, "
        "carbon_grams_per_1k_tokens, region) VALUES ('bench1', '01J9TEST', 'balanced', 'fp16', "
        "100, 1000, 42.5, 120, 1500.0, 45.0, 1, 1700000000000, 0.85, 0.92, 0.01, 'trusted', 0, 0, "
        "0.005, 0.12, 'us-east-1')"
    )

    # Sample INSERT into embedding_sources + embedding_vectors (RAG pipeline, migration 0007 + 0017).
    cur.execute(
        "INSERT INTO embedding_sources (id, source_type, source_uri, chunk_idx, content, model_id, embedded_at) "
        "VALUES ('src1', 'file', '/tmp/doc.txt', 0, 'hello world', 'edgeforge-hashing-768', 1700000000000)"
    )
    # 4 f32 values = 16 bytes (little-endian)
    cur.execute(
        "INSERT INTO embedding_vectors (id, source_id, model_id, dim, vector, norm, created_at) "
        "VALUES ('vec1', 'src1', 'edgeforge-hashing-768', 4, X'0000803F000000BF0000403F0000003F', 1.0, 1700000000000)"
    )

    conn.commit()

    cur.execute("SELECT COUNT(*) FROM meta")
    meta_count = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM models")
    models_count = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM jobs")
    jobs_count = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM benchmark_runs")
    bench_count = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM embedding_sources")
    emb_src_count = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM embedding_vectors")
    emb_vec_count = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM _sqlx_migrations")
    applied_count = cur.fetchone()[0]

    print()
    print(f"  _sqlx_migrations rows: {applied_count}")
    print(f"  meta rows:              {meta_count}")
    print(f"  models rows:            {models_count}")
    print(f"  jobs rows:              {jobs_count}")
    print(f"  benchmark_runs rows:    {bench_count}")
    print(f"  embedding_sources rows: {emb_src_count}")
    print(f"  embedding_vectors rows: {emb_vec_count}")
    print(f"  tables present:         {len(actual_tables)}")

    conn.close()


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <migrations_dir>", file=sys.stderr)
        return 2
    migrations_dir = Path(sys.argv[1])
    try:
        apply_migrations(migrations_dir)
    except Exception as e:
        print(f"\nVALIDATION FAILED: {e}", file=sys.stderr)
        return 1
    print("\nAll migrations applied cleanly and sample INSERTs succeeded.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
