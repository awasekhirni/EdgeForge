#!/usr/bin/env bash
set -euo pipefail
echo "=== EdgeForge Dev Bootstrap ==="
rustup component add rustfmt clippy
cargo install cargo-deny 2>/dev/null || true
./scripts/build-llama.sh
cargo build
cargo test --workspace
echo "Done."
