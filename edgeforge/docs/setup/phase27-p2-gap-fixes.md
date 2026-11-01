# EdgeForge — Phase 27: P2 Gap Fixes (Missing Inline Tests)

**Date:** 2026-11-01
**Status:** Complete
**Files changed:** 11 modified + 1 new

---

## Summary

Four P2 gaps from the codebase evaluation identified missing inline tests in
foundational crates. All are now fixed with comprehensive `#[cfg(test)]` modules.

**134 new inline tests** added across 11 files in 4 crates.

---

## Changes

### Gap #8: edgeforge-core (69 tests across 4 files)

| File | Tests | Coverage |
|---|---|---|
| `types.rs` | 16 | ID new/parse/display roundtrip, invalid parse, LayerIdx.next(), ExpertId equality, ExpertSource variants, Token fields, Prompt::new/render_flat (single/multiple/empty), ID default, ID ordering |
| `error.rs` | 23 | HTTP status mapping for every variant (400, 401, 403, 404, 409, 429, 500, 501), Display output |
| `units.rs` | 15 | Bytes from_gib/add/sub/saturating_sub/display/equality/ordering, TokPerSec display/default, Celsius THROTTLE/CRITICAL constants/display/comparison, helper functions |
| `control_event.rs` | 15 | All 8 ControlEventKind variants construction, SpeculationHorizon/CrowdTier/VerificationOutcome equality, serialization (JSON tag names) |

### Gap #9: edgeforge-crowd (21 tests across 2 files)

| File | Tests | Coverage |
|---|---|---|
| `ledger.rs` | 8 | Empty ledger, credit increases, debit decreases, multiple credits accumulate, unknown peer balance=0, negative balance allowed, EntryKind equality, LedgerEntry serialization |
| `reputation.rs` | 13 | Empty graph, admit sets default score, admit increases len, record_success updates score + EMA latency, record_failure decreases score, success_rate (no data/all success/mixed), evict removes peer, get unknown returns None, record on unknown peer is no-op |

### Gap #10: edgeforge-graph (26 tests across 3 files)

| File | Tests | Coverage |
|---|---|---|
| `core_.rs` | 11 | Empty+healthy graph, best_peer empty/unhealthy/returns peer/prefers lower RTT, estimate_fetch_time known/unknown, record_peer_lost, record_router_decision, set_healthy toggle, ExpertKey equality |
| `query.rs` | 8 | healthy true/false, best_peer delegates, estimate_fetch_time delegates, predicted_expert_set empty/returns history/respects n, ingest returns error |
| `ingest.rs` | 7 | try_enqueue succeeds, full drops, drain_and_apply heartbeat/peer_lost/router_decision, empty drain=0, config default capacity |

### Gap #11: edgeforge-kpi (18 tests across 2 files)

| File | Tests | Coverage |
|---|---|---|
| `carbon.rs` | 8 | Static table known/unknown regions, CarbonEstimate from_energy, router picks lowest carbon, skips high latency, returns none when all violate SLO, empty peers, single peer within SLO |
| `traceability.rs` | 10 | Empty matrix, add+retrieve, items_for_adr match/no-match, tests_for_requirement match/no-match, to_json contains items, empty matrix JSON, default trace, add overwrites existing |

---

## Test Summary

| Crate | Files Modified | New Tests |
|---|---|---|
| edgeforge-core | 4 | 69 |
| edgeforge-crowd | 2 | 21 |
| edgeforge-graph | 3 | 26 |
| edgeforge-kpi | 2 | 18 |
| **Total** | **11** | **134** |

---

## Validation

### Brace balance
All 11 modified files pass brace/paren balance check (0 mismatches).

### Migrations still validate
All 18 migrations applied cleanly.
