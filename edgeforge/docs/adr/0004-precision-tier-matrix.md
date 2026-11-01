# ADR-004: Precision Tier Matrix & Hard-Block Policy

**Status:** Approved
**Date:** 2026-10-31

## Decision

All precision tiers (FP32/FP16/BF16/Q8/Q4/Q3) are supported. The UI hard-blocks any configuration where `free_ssd < 1.2 × model_size` at install time with a plain-English message recommending a lower precision or larger SSD.

## Precision Tier Matrix

| Precision | Bytes/param | 1T Model Size | Min SSD |
|---|---|---|---|
| FP32 | 4 | 4.0 TB | 6 TB |
| FP16/BF16 | 2 | 2.0 TB | 2 TB (zero headroom) |
| Q8 | 1 | 1.0 TB | 2 TB |
| Q4 | 0.5 | 500 GB | 2 TB |
| Q3 | 0.375 | 375 GB | 2 TB |
