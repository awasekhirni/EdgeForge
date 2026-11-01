# ADR-027: Hybrid Post-Quantum Mesh Transport
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
Hybrid PQC in QUIC mTLS: X25519 + ML-KEM-768 (KEM), Ed25519 + ML-DSA-65 (sig). Beta channel from day one (2026); stable 2028; PQC-only 2031+. Crypto-agility layer (KemAlgorithm/SigAlgorithm traits). /v1/crypto/inventory API.

## Migration Roadmap
| Period | Stable | Beta |
|---|---|---|
| 2026–2028 | Classical | Hybrid |
| 2028–2031 | Hybrid mandatory | PQC-only |
| 2031+ | PQC-only | PQC-only |
