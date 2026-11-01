# ADR-015: Multi-Horizon Speculation with Accuracy SLA
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v2.0

## Decision
Three-horizon speculation: H1 (router logit, >90%), H2 (surrogate/draft, 60-85%), H3 (graph sequence query, 40-70%). Quality preset SLA: ≥99.9% token match vs exact-routing FP16. Surrogate hard-disabled under Quality (R-19).
