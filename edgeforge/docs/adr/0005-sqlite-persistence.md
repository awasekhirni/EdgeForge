# ADR-005: SQLite + sqlite-vss Persistence

**Status:** Approved
**Date:** 2026-10-31

## Decision

Use SQLite (with the `sqlite-vss` extension for vector search) as the sole persistence layer. SQLite is zero-config, air-gapped-friendly, and sufficient for the single-node desktop use case.

## Schema

16 migrations covering: models, jobs, conversations, pool nodes, benchmarks, embeddings (sqlite-vss), peer reputation, verification, incentive ledger, prompt audit, KPI QPPOs, traceability, autonomic, and crypto inventory.

## Consequences

The `sqlite-vss` extension is loaded at runtime via `rusqlite::load_extension`. If unavailable, the `/v1/embeddings` and RAG endpoints degrade gracefully with a warning.
