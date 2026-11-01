# ADR-028: Hermetic Reproducible Builds (Nix)
**Status:** Approved | **Date:** 2026-10-31 | **Version:** v4.0

## Decision
Nix flake pins Rust 1.82 + all system libs by hash. Build archaeology container archived with every release. Bit-for-bit reproducible artifacts verified by CI on two platforms. Cargo.lock committed.
