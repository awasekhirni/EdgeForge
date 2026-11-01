# ADR-024: Legacy-First Core Design (Zero-Dependency, no_std)
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
edgeforge-core-legacy: #![no_std] + #![forbid(unsafe_code)] + zero external deps. Contains ModelId, Precision, LegacyError (stable codes), ComputeSubstrate trait. Reimplementable in any language from the specification alone.

## 50-Year Durability
If the rest of EdgeForge is rewritten in a future language, this crate's spec is the authoritative source for the data model and error taxonomy.
