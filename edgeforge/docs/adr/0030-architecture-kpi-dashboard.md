# ADR-030: Architecture KPI Dashboard + CMMI QPPOs
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
8 frozen QPPOs with UCL/LCL: MTTR deploys, regression escape rate, defect density, Kani coverage, build time p95, test flake rate, unsafe LOC, dependency freshness. Bidirectional traceability matrix as CI gate ([trace(...)] required on every public item). Automated CAR workflow on violation.

## CMMI Compliance
| Area | Implementation |
|---|---|
| OPP | KPI Dashboard with QPPOs |
| CAR | MAPE-K loop (ADR-029) |
| DAR | Dual implementations (ADR-032) |
| TS | Kani proofs (ADR-025) |
| PI | Traceability matrix CI gate |
| VV | Formal verification + chaos |
| RSK | Streaming graph feeds risk signals |
| CM | Nix + crypto inventory |
