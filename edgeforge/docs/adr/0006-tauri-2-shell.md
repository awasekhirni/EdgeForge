# ADR-006: Tauri 2 Desktop Shell

**Status:** Approved
**Date:** 2026-10-31

## Decision

Use Tauri 2 as the desktop shell. Tauri is Rust-native, has the smallest binary footprint, and provides a webview-based UI without shipping a full Chromium instance.

## Architecture

- `edgeforge-app` crate: Tauri entry point, IPC commands, menu.
- `ui/` directory: React + Tailwind + Zustand frontend.
- Communication: Tauri IPC → L5 Orchestration (never touches L4 directly).
