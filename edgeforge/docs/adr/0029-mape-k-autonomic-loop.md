# ADR-029: MAPE-K Autonomic Loop with Antifragile Learning
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
MAPE-K (Monitor-Analyze-Plan-Execute over Knowledge) loop. SPC (Welford's) distinguishes common-cause vs special-cause variation. Antifragile: on success, generates regression test + Kani harness, commits to CI suite. Chaos engineering in CI as release gate.
