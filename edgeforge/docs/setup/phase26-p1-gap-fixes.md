# EdgeForge — Phase 26: P1 Gap Fixes (IOCP/dispatch_io + Speculation cleanup)

**Date:** 2026-11-01
**Status:** Complete
**Files changed:** 3 modified + 1 new

---

## Summary

Two remaining P1 gaps from the codebase evaluation:

1. **Windows IOCP + macOS dispatch_io backends used `unimplemented!()`** — would panic at runtime instead of falling back to `BlockingIo`
2. **Speculation H1/H2 had misleading "Phase 5" comments** — the functions were already implemented but the comments made them look like stubs; also missing comprehensive H2 tests

Both are now fixed. Webhooks (#5) was already fixed in Phase 25.

---

## Fix #6: IOCP + dispatch_io — Replace `unimplemented!()` with BlockingIo fallback

### Before
```rust
// iocp.rs
async fn read_at(...) -> Result<AlignedBuffer, IoError> {
    unimplemented!("IOCP read_at — Phase 5+")  // ← panics at runtime!
}

// dispatch_io.rs
async fn read_at(...) -> Result<AlignedBuffer, IoError> {
    unimplemented!("dispatch_io read_at — Phase 5+")  // ← panics at runtime!
}
```

### After
Both `Iocp` (Windows) and `DispatchIo` (macOS) now own a `BlockingIo` fallback
and delegate all `PlatformIo` methods to it — same pattern as `IoUring` on Linux.

```rust
pub struct Iocp {
    fallback: crate::platform::BlockingIo,
}

async fn read_at(&self, file: &DirectFile, offset: u64, len: usize) -> Result<AlignedBuffer, IoError> {
    self.fallback.read_at(file, offset, len).await  // ← correct, no panic
}
```

Both files now have comprehensive module docs explaining the fallback strategy,
all methods are documented with `## Parameters` / `## Returns` sections, and
all calls log via `tracing::debug!`.

---

## Fix #7: Speculation H1/H2 — Remove misleading comments, add tests

### Before
H1 and H2 were **already implemented** with real logic (H1 sorts logits by value
and takes top-k; H2 collects draft model predictions per layer). However:
- The `// Phase 5:` comments made them look like stubs
- The `#[must_use]` attribute had a typo (`#ust_use]`)
- H2 had no dedicated tests

### After
- Removed all `// Phase 5:` comments from H1 and H2
- Fixed the `#[must_use]` attribute
- Added comprehensive doc comments with `## Purpose`, `## Parameters`, `## Returns`, `## Algorithm` sections
- Renamed `_router_logits` → `router_logits` (it's used, not ignored)
- Added 8 new tests:
  - `h1_predicts_all_when_top_k_exceeds_len` — top_k > logits.len() returns all
  - `h1_predicts_empty_logits` — empty input returns empty
  - `h1_predicts_sorted_descending` — verifies sort order
  - `h2_predicts_future_layers` — verifies layer numbering (N+1, N+2)
  - `h2_respects_depth_limit` — depth truncates the output
  - `h2_empty_draft_returns_empty` — empty input
  - `h2_empty_layers_skipped` — empty layers in the draft produce no predictions

---

## Changes

### Modified Files (3)

| File | Change |
|---|---|
| `crates/edgeforge-storage/src/platform/iocp.rs` | Full rewrite: `Iocp` now owns `BlockingIo` fallback; all methods delegate; `unimplemented!()` removed; comprehensive docs + logging |
| `crates/edgeforge-storage/src/platform/dispatch_io.rs` | Full rewrite: `DispatchIo` now owns `BlockingIo` fallback; all methods delegate; `unimplemented!()` removed; comprehensive docs + logging |
| `crates/edgeforge-runtime/src/speculation.rs` | Removed misleading "Phase 5" comments; fixed `#[must_use]` typo; renamed `_router_logits` → `router_logits`; added comprehensive doc comments; added 8 new tests (4 H1 + 4 H2) |

### New Files (1)

| File | Purpose |
|---|---|
| `docs/setup/phase26-p1-gap-fixes.md` | This change log |

---

## Test Summary

| File | Tests |
|---|---|
| `speculation.rs` | 15 (was 7, added 8: 4 H1 + 4 H2) |

---

## Validation

### No `unimplemented!()` calls
```bash
grep -rn "unimplemented!" crates/edgeforge-storage/src/platform/iocp.rs crates/edgeforge-storage/src/platform/dispatch_io.rs
# (no output — no unimplemented! calls)
```

### No misleading "Phase 5" comments
```bash
grep -c "Phase 5" crates/edgeforge-runtime/src/speculation.rs
# 0
```

### Brace balance
All 3 modified files pass brace/paren balance check.

### Migrations still validate
```bash
python3 scripts/validate_migrations.py resources/migrations
# All 18 migrations applied cleanly.
```
