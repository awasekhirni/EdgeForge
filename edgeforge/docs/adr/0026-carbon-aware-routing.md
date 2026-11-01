# ADR-026: Carbon-Aware Inference Routing
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
CarbonAwareRouter routes expert fetches to lowest-carbon peer satisfying latency SLO. Static IEA 2024 carbon table for air-gapped; opt-in API for connected. Energy-per-token as QPPO #9 (no >10% regression per release).

## Reference
CarbonRouter (2024) — routing LLM queries to reduce carbon emissions.
