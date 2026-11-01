#!/usr/bin/env bash
set -euo pipefail
echo "=== EdgeForge Installer ==="
if ! command -v cargo &>/dev/null; then curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable; source "$HOME/.cargo/env"; fi
cargo build --release --bin edgeforge
echo "Done. Run: ./target/release/edgeforge serve"
