# ADR-032: Multiple Independent Implementations of Critical Paths
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
Dual implementations for critical paths:
| Component | Impl A (primary) | Impl B (fallback) |
|---|---|---|
| InferenceEngine | LlamaBackend (llama.cpp FFI) | SsdEngine (custom Rust) |
| ManifestVerifier | ring-based | RustCrypto-based |

Both pass the same trait test suite. Selection re-evaluated annually via formal DAR criteria.
