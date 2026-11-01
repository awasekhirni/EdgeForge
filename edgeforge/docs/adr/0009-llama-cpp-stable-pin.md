# ADR-009: llama.cpp Pinned to Latest Stable Tag

**Status:** Approved
**Date:** 2026-10-31

## Decision

`llama.cpp` is tracked as a git submodule pinned to the latest stable tag at release time. The pin is re-evaluated per release.

## Build

`scripts/build-llama.sh` detects the host triple and CPU ISA (AVX2/AVX-512/NEON), then builds `llama.cpp` as a static library via CMake. The `edgeforge-runtime-sys` `build.rs` finds the artifacts automatically.
