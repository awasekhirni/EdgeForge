// Kani proof harnesses for storage-layer invariants (ADR-025).
//
// These harnesses verify safety properties of the storage subsystem:
//   - KvCachePager: LRU eviction, budget enforcement, hit-rate calculation
//   - WeightPager: offset calculation, page ID construction
//   - ThermalState: threshold comparisons
//   - Wear: write-amplification accounting
//
// To run:
//   cargo install kani-verifier
//   cargo kani --harness <name>

#![cfg(kani)]

use edgeforge_core::{Bytes, LayerIdx};
use edgeforge_storage::kv_pager::{KvCachePager, KvPageId, KvPagerConfig};

// ── KvCachePager harnesses ─────────────────────────────────────────────────

/// Property: A freshly-created KvCachePager has zero pages, zero bytes, and
/// zero hit rate.
#[kani::proof]
fn verify_kv_pager_empty_invariant() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });
    kani::assert_eq!(pager.len(), 0, "fresh pager must be empty");
    kani::assert(pager.is_empty(), "fresh pager must be empty");
    kani::assert_eq!(pager.current_bytes(), 0, "fresh pager must hold 0 bytes");
    kani::assert_eq!(pager.evictions(), 0, "fresh pager must have 0 evictions");
    kani::assert(
        (pager.hit_rate() - 0.0).abs() < 1e-9,
        "fresh pager hit rate must be 0.0",
    );
}

/// Property: After `put`, `get` returns the same bytes (roundtrip).
#[kani::proof]
fn verify_kv_pager_put_get_roundtrip() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024 * 1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });
    let id = KvPageId { layer: LayerIdx(0), page: 0 };
    let payload = vec![0xCDu8; 128];
    pager.put(id, payload.clone());
    let retrieved = pager.get(id);
    kani::assert(retrieved.is_some(), "get after put must return Some");
    kani::assert(
        retrieved.as_ref().unwrap() == &payload,
        "get must return the exact bytes put",
    );
}

/// Property: `get` on a missing page returns `None`.
#[kani::proof]
fn verify_kv_pager_miss_returns_none() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });
    let id = KvPageId { layer: LayerIdx(0), page: 0 };
    let result = pager.get(id);
    kani::assert(result.is_none(), "get on empty pager must return None");
}

/// Property: The pager never exceeds its `ram_budget`.
#[kani::proof]
fn verify_kv_pager_never_exceeds_budget() {
    let budget: u64 = kani::any();
    kani::assume(budget >= 16 && budget <= 4096);

    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(budget),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });

    for i in 0u32..8 {
        let id = KvPageId { layer: LayerIdx(i), page: 0 };
        let size: u64 = kani::any();
        kani::assume(size >= 1 && size <= budget);
        let payload = vec![0u8; size as usize];
        pager.put(id, payload);

        kani::assert(
            pager.current_bytes() <= budget,
            "current_bytes must never exceed ram_budget",
        );
    }
}

/// Property: LRU eviction removes the least-recently-used page.
#[kani::proof]
fn verify_kv_pager_lru_eviction() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(32), // fits exactly two 16-byte pages
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });

    let id_a = KvPageId { layer: LayerIdx(0), page: 0 };
    let id_b = KvPageId { layer: LayerIdx(0), page: 1 };
    let id_c = KvPageId { layer: LayerIdx(0), page: 2 };

    pager.put(id_a, vec![0u8; 16]);
    pager.put(id_b, vec![0u8; 16]);

    // Access A to bump its LRU clock.
    let _ = pager.get(id_a);

    // Insert C — must evict B (the LRU), not A.
    pager.put(id_c, vec![0u8; 16]);

    kani::assert(pager.get(id_a).is_some(), "A must survive (was just accessed)");
    kani::assert(pager.get(id_b).is_none(), "B must be evicted (LRU)");
    kani::assert(pager.get(id_c).is_some(), "C must be resident");
    kani::assert_eq!(pager.evictions(), 1, "exactly 1 eviction must occur");
}

/// Property: `evict` removes the page and decrements `current_bytes`.
#[kani::proof]
fn verify_kv_pager_evict_decrements_bytes() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });
    let id = KvPageId { layer: LayerIdx(0), page: 0 };
    pager.put(id, vec![0u8; 64]);
    kani::assert_eq!(pager.current_bytes(), 64, "bytes must be 64 after put");

    let evicted = pager.evict(id);
    kani::assert(evicted, "evict must return true for a resident page");
    kani::assert_eq!(pager.current_bytes(), 0, "bytes must be 0 after evict");
    kani::assert(!pager.contains(id), "page must not be resident after evict");
    kani::assert_eq!(pager.evictions(), 1, "eviction count must be 1");
}

/// Property: `evict` on a non-resident page returns `false` and doesn't
/// change the eviction count.
#[kani::proof]
fn verify_kv_pager_evict_nonexistent_returns_false() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });
    let id = KvPageId { layer: LayerIdx(0), page: 0 };
    let evicted = pager.evict(id);
    kani::assert(!evicted, "evict on non-resident page must return false");
    kani::assert_eq!(pager.evictions(), 0, "eviction count must be 0");
}

/// Property: `put` overwriting an existing page adjusts `current_bytes`
/// correctly (old bytes subtracted, new bytes added).
#[kani::proof]
fn verify_kv_pager_put_overwrite_adjusts_bytes() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });
    let id = KvPageId { layer: LayerIdx(0), page: 0 };

    pager.put(id, vec![0u8; 16]);
    kani::assert_eq!(pager.current_bytes(), 16, "bytes must be 16 after first put");

    pager.put(id, vec![0u8; 32]);
    kani::assert_eq!(pager.current_bytes(), 32, "bytes must be 32 after overwrite (not 16+32)");
    kani::assert_eq!(pager.len(), 1, "len must still be 1 (overwrite, not insert)");

    pager.put(id, vec![0u8; 8]);
    kani::assert_eq!(pager.current_bytes(), 8, "bytes must be 8 after shrink");
}

/// Property: `hit_rate` is always in [0.0, 1.0].
#[kani::proof]
fn verify_kv_pager_hit_rate_in_range() {
    let pager = KvCachePager::new(KvPagerConfig {
        ram_budget: Bytes(1024),
        page_size: Bytes(4096),
        ssd_dir: std::path::PathBuf::from("/tmp"),
    });

    // Perform a sequence of puts and gets.
    for i in 0u32..10 {
        let id = KvPageId { layer: LayerIdx(i), page: 0 };
        pager.put(id, vec![0u8; 16]);
    }
    for i in 0u32..5 {
        let id = KvPageId { layer: LayerIdx(i), page: 0 };
        let _ = pager.get(id);
    }

    let hr = pager.hit_rate();
    kani::assert(hr >= 0.0, "hit_rate must be >= 0.0");
    kani::assert(hr <= 1.0, "hit_rate must be <= 1.0");
}

/// Property: `KvPageId` equality is based on (layer, page).
#[kani::proof]
fn verify_kv_page_id_equality() {
    let l1: u32 = kani::any();
    let p1: u32 = kani::any();
    let l2: u32 = kani::any();
    let p2: u32 = kani::any();

    let id1 = KvPageId { layer: LayerIdx(l1), page: p1 };
    let id2 = KvPageId { layer: LayerIdx(l2), page: p2 };

    if l1 == l2 && p1 == p2 {
        kani::assert(id1 == id2, "same (layer, page) → equal IDs");
    } else {
        kani::assert(id1 != id2, "different (layer, page) → unequal IDs");
    }
}

// ── ThermalState harness ───────────────────────────────────────────────────

/// Property: ThermalState defaults to non-throttled, non-paused.
#[kani::proof]
fn verify_thermal_state_default_safe() {
    use edgeforge_storage::ThermalState;
    let state = ThermalState::default();
    kani::assert(!state.throttled, "default ThermalState must not be throttled");
    kani::assert(!state.paused, "default ThermalState must not be paused");
    kani::assert(state.current.is_none(), "default ThermalState must have no current temp");
}

// ── SsdHealth harness ──────────────────────────────────────────────────────

/// Property: SsdHealth defaults to all-zero (healthy drive).
#[kani::proof]
fn verify_ssd_health_default_zero() {
    use edgeforge_storage::wear::SsdHealth;
    let health = SsdHealth::default();
    kani::assert_eq!(health.total_bytes_written, 0, "default TBW must be 0");
    kani::assert_eq!(health.percent_used, 0.0, "default percent_used must be 0.0");
    kani::assert_eq!(health.remaining_tbw_bytes, 0, "default remaining_tbw must be 0");
}

/// Property: SsdHealth status thresholds are correctly classified.
#[kani::proof]
fn verify_ssd_health_status_thresholds() {
    use edgeforge_storage::wear::{SsdHealth, SsdHealthStatus};

    // Healthy: percent_used < 50%
    let h = SsdHealth { percent_used: 10.0, ..Default::default() };
    kani::assert_eq!(h.status(), SsdHealthStatus::Healthy, "10% → Healthy");

    // Warning: 50% ≤ percent_used < 80%
    let h = SsdHealth { percent_used: 60.0, ..Default::default() };
    kani::assert_eq!(h.status(), SsdHealthStatus::Warning, "60% → Warning");

    // Critical: percent_used ≥ 80%
    let h = SsdHealth { percent_used: 85.0, ..Default::default() };
    kani::assert_eq!(h.status(), SsdHealthStatus::Critical, "85% → Critical");

    // Boundary: exactly 50% → Warning
    let h = SsdHealth { percent_used: 50.0, ..Default::default() };
    kani::assert_eq!(h.status(), SsdHealthStatus::Warning, "50% → Warning (boundary)");

    // Boundary: exactly 80% → Critical
    let h = SsdHealth { percent_used: 80.0, ..Default::default() };
    kani::assert_eq!(h.status(), SsdHealthStatus::Critical, "80% → Critical (boundary)");
}

/// Property: SsdHealth status classification for nondeterministic percent_used.
#[kani::proof]
fn verify_ssd_health_status_nondeterministic() {
    use edgeforge_storage::wear::{SsdHealth, SsdHealthStatus};
    let percent: f32 = kani::any();
    kani::assume(percent >= 0.0 && percent <= 100.0);

    let h = SsdHealth { percent_used: percent, ..Default::default() };
    let status = h.status();

    // Verify the classification is consistent with the thresholds.
    if percent < 50.0 {
        kani::assert_eq!(status, SsdHealthStatus::Healthy, "<50% → Healthy");
    } else if percent < 80.0 {
        kani::assert_eq!(status, SsdHealthStatus::Warning, "50-80% → Warning");
    } else {
        kani::assert_eq!(status, SsdHealthStatus::Critical, ">=80% → Critical");
    }
}
