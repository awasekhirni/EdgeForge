# ADR-013: Stable + Beta Release Channels

**Status:** Approved
**Date:** 2026-10-31

## Decision

Two release channels only (no nightly):

- **Stable** (default) — production-ready releases.
- **Beta** — opt-in; latest features; rollback button is one click away.

## Auto-Update

Signed auto-update via OS-native notarization (macOS) / Authenticode (Windows) / minisign (Linux). Delta patches. Auto-install is off by default — EdgeForge prompts before each install.

## Rollback

The previous binary is retained; one-click rollback (F-UPD-04).
