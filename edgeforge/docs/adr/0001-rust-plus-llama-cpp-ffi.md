# ADR-001: Rust Workspace + llama.cpp FFI

**Status:** Approved
**Date:** 2026-10-31

## Context

EdgeForge needs to run frontier-scale MoE models (1T parameters) locally using an NVMe SSD as an extended memory tier. The primary tech stack is Rust. The inference engine must leverage `llama.cpp` for its mature GGUF support, quantization pipeline, and speculative decoding.

## Decision

**Path 2: Rust workspace for L1–L3, L5, L6; llama.cpp C++ FFI confined to L4 only.**

- All layers except L4 are pure Rust.
- L4 (`edgeforge-runtime-sys`) contains the only `unsafe` FFI to `llama.cpp`.
- `cargo-deny` blocks `unsafe` outside `edgeforge-runtime-sys` and `edgeforge-storage/platform/*`.
- The C++ blast radius is contained to one crate.

## Consequences

**Positive:** Matches the design doc; preserves llama.cpp ecosystem maturity; contains C++ to one crate.
**Negative:** Accepts a C++ build dependency; requires `build.rs` + cmake.
