# ADR-002: 2 TB NVMe Gen4 Minimum SSD

**Status:** Approved
**Date:** 2026-10-31

## Context

A 1T-parameter MoE model at FP16 requires 2 TB of storage. At Q4 it requires 500 GB. The user directive mandates FP16 as the default precision for accuracy, which necessitates a 2 TB minimum SSD.

## Decision

**Minimum SSD = 2 TB NVMe Gen4.** The UI hard-blocks any configuration where `free_ssd < 1.2 × model_size` at install time.

## Storage Math

| Precision | 1T Model Size | Fits 2 TB? |
|---|---|---|
| FP32 | 4.0 TB | ❌ |
| FP16 (default) | 2.0 TB | ✅ (zero headroom) |
| Q8 | 1.0 TB | ✅ |
| Q4 | 500 GB | ✅ |

## Consequences

FP16 on 2 TB has zero download headroom — the UI warns and recommends 4 TB for production FP16 use.
