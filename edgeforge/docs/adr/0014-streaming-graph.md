# ADR-014: In-Memory Streaming Graph at L3.5

**Status:** Approved | **Date:** 2026-10-31 | **Version:** v2.0

## Decision
Derived, in-memory property graph at L3.5. Nodes = experts, peers, sequences. Edges = residency, RTT, routing history. <10µs p99 query contract. Bounded-channel ingest with load shedding (R-18). Graph is derived only — falls back to deterministic SSD behavior on failure.

## Invariants
1. Graph is derived only; never stores authoritative state.
2. `healthy() == false` → callers MUST fall back to SSD.
3. Graph MUST NOT depend on edgeforge-runtime or edgeforge-storage.
