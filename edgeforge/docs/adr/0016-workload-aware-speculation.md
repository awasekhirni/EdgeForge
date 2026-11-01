# ADR-016: Workload-Aware Speculation Strategy
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v2.0

## Decision
Auto-detect workload (Code → ngram-simple, Structured → ngram-map-k, NL → draft, Mixed → conservative draft). Maps to llama.cpp's --spec-type.
