# ADR-025: Formal Verification Gate (Kani + Verus)
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
Kani model checking for every unsafe block (≥90% coverage, QPPO #4). Verus contracts for PlatformIo (safety, liveness, cancellation). CI release gate. KAPILOT (Phase 4+) for LLM-assisted harness generation.

## References
- Kani: https://github.com/model-checking/kani
- Verus: https://github.com/verus-lang/verus
