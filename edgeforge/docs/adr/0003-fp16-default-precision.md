# ADR-003: FP16 Default Precision

**Status:** Approved
**Date:** 2026-10-31

## Context

The user directive mandates FP16 as the default precision for 1T-parameter models to ensure maximum accuracy for research and accuracy-critical workloads.

## Decision

**FP16 is the default precision** for the `Quality` preset (the default preset). The precision tier matrix:

| Preset | Precision | Use Case |
|---|---|---|
| Quality (default) | FP16 | Research, accuracy-critical |
| Balanced | Q8 | Daily driver |
| Fast | Q4 | Maximum throughput |
| Max-Quality | FP32 | Opt-in; requires ≥6 TB SSD |

## Consequences

FP16 on 2 TB NVMe has zero headroom (ADR-002). The `Quality` preset carries a hard accuracy SLA: ≥99.9% token match vs exact-routing FP16 on the benchmark suite. Surrogate approximation is hard-disabled under Quality (R-19).
