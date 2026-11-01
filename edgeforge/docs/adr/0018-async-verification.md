# ADR-018: Asynchronous Verification with Retroactive Correction
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v2.0

## Decision
50ms verification deadline. X-EdgeForge-Verification header: verified/pending/corrected. If verification fails within 500ms, emit correction chunk. Tier 3 verification never blocks the token stream (R-21).
