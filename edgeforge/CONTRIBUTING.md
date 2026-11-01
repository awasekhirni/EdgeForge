# Contributing to EdgeForge

Thank you for your interest in contributing to EdgeForge! This document covers development setup, coding standards, and the PR process.

## Development Setup

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
rustup component add rustfmt clippy rust-src
```

### 2. Install Build Tools

**macOS:**
```bash
xcode-select --install
brew install cmake git pkg-config
```

**Ubuntu:**
```bash
sudo apt install -y build-essential cmake git pkg-config libssl-dev libsqlite3-dev libclang-dev llvm
```

**Windows:**
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools"
winget install Git.Git Kitware.CMake
```

### 3. Clone and Build

```bash
git clone --recurse-submodules https://github.com/your-org/edgeforge.git
cd edgeforge
cargo build
cargo test --workspace
```

### 4. (Optional) Dev Bootstrap

```bash
./scripts/bootstrap-dev.sh
```

This installs `cargo-deny`, `cargo-outdated`, and runs the full build + test suite.

## Coding Standards

### Formatting

```bash
cargo fmt --all -- --check
```

All code must be formatted with `rustfmt`. The config is in `rustfmt.toml`.

### Linting

```bash
cargo clippy --workspace -- -D warnings
```

No clippy warnings are permitted. The config is in `clippy.toml` (msrv = 1.82).

### Testing

```bash
cargo test --workspace
```

All 55 tests must pass. New code must include tests.

### Dependency Hygiene

```bash
cargo deny check
```

- No `openssl` (use `rustls` instead — ADR-001)
- No duplicate crate versions (warned)
- License allowlist enforced

### Traceability (ADR-030)

Every public API item must carry a `[trace(...)]` attribute linking it to ADRs, requirements, tests, and CMMI process areas. CI fails any PR that introduces a public item without a trace.

### `unsafe` Policy

`unsafe` is permitted only in:
- `edgeforge-runtime-sys` (FFI to llama.cpp)
- `edgeforge-storage/src/platform/` (platform I/O)

All `unsafe` blocks must have Kani proof harnesses (ADR-025, ≥90% coverage QPPO).

## PR Process

1. **Fork** the repository.
2. **Create a branch** from `main` (or `beta` for beta-channel features).
3. **Write code** with tests and `[trace(...)]` attributes.
4. **Run checks**: `cargo fmt && cargo clippy && cargo test && cargo deny check`.
5. **Open a PR** with a clear description of what changed and why.
6. **Address review feedback**.
7. **Merge** once CI passes and a maintainer approves.

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(api): add SSE streaming to /v1/chat/completions
fix(storage): fix LRU eviction order in expert cache
docs(adr): write ADR-025 formal verification gate
test(runtime): add e2e integration tests for MockInferenceEngine
```

## Architecture Decision Records (ADRs)

All architectural decisions are recorded in `docs/adr/`. When making a significant architectural decision:

1. Create a new ADR file: `docs/adr/00NN-title.md`
2. Use the ADR template (see any existing ADR for the format).
3. Include: Context, Decision, Consequences, Architectural Invariants.
4. Link the ADR from relevant code comments.

## Release Process

1. Tag the release: `git tag v0.X.Y` (stable) or `v0.X.Y-beta` (beta).
2. CI builds the release artifacts for macOS, Windows, and Linux.
3. CI builds the Python wheels for cp39–cp313.
4. The release is published to GitHub Releases.
5. The build archaeology container is archived (ADR-028).

## Getting Help

- **Issues:** https://github.com/your-org/edgeforge/issues
- **ADRs:** `docs/adr/`
- **Architecture docs:** `docs/architecture/`
- **Setup guides:** `docs/setup/`
