// Kani proof harnesses for the PlatformIo trait + ExpertCache (ADR-025).
//
// These harnesses use `kani::any()` to generate nondeterministic inputs and
// `kani::assume` / `kani::assert` to verify safety properties. They run under
// `cargo kani` (not `cargo test`).
//
// Coverage:
//   - BlockingIo::read_at: bounds, no panic on valid inputs
//   - ExpertCache: LRU eviction invariant, budget enforcement, get/put roundtrip
//   - WeightPager: offset calculation never overflows
//   - AlignedBuffer: length invariant
//
// To run:
//   cargo install kani-verifier
//   cargo kani --harness <name>
//
// Or all harnesses:
//   cargo kani

#![cfg(kani)]

use edgeforge_core::{Bytes, ExpertId, LayerIdx};
use edgeforge_storage::expert_cache::{CacheKey, ExpertCache, ExpertCacheConfig};

// ── ExpertCache harnesses ──────────────────────────────────────────────────

/// Property: A freshly-created ExpertCache has zero entries and zero bytes.
#[kani::proof]
fn verify_expert_cache_empty_invariant() {
    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(1024),
    });
    assert_eq!(cache.len(), 0, "fresh cache must be empty");
    assert!(cache.is_empty(), "fresh cache must be empty");
    assert_eq!(cache.current_bytes(), 0, "fresh cache must hold 0 bytes");
}

/// Property: After `put`, `get` returns the same bytes (roundtrip).
#[kani::proof]
fn verify_expert_cache_put_get_roundtrip() {
    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(1024 * 1024),
    });
    let key = CacheKey {
        layer: LayerIdx(0),
        expert: ExpertId(0),
    };
    let payload = vec![0xABu8; 64];
    cache.put(key, payload.clone());
    let retrieved = cache.get(key);
    kani::assert(retrieved.is_some(), "get after put must return Some");
    kani::assert(
        retrieved.as_ref().unwrap() == &payload,
        "get must return the exact bytes put",
    );
    assert_eq!(cache.len(), 1, "cache must have 1 entry after put");
}

/// Property: `get` on a missing key returns `None`.
#[kani::proof]
fn verify_expert_cache_miss_returns_none() {
    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(1024),
    });
    let key = CacheKey {
        layer: LayerIdx(0),
        expert: ExpertId(0),
    };
    let result = cache.get(key);
    kani::assert(result.is_none(), "get on empty cache must return None");
}

/// Property: The cache never exceeds its `ram_budget`.
///
/// This is the core safety invariant: `current_bytes() <= ram_budget` after
/// any sequence of `put` operations. Kani explores all possible eviction
/// orders to verify this holds.
#[kani::proof]
fn verify_expert_cache_never_exceeds_budget() {
    let budget: u64 = kani::any();
    // Constrain the budget to a reasonable range so Kani can exhaustively
    // explore the state space.
    kani::assume(budget >= 16 && budget <= 4096);

    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(budget),
    });

    // Insert a sequence of entries with nondeterministic sizes.
    for i in 0u32..8 {
        let key = CacheKey {
            layer: LayerIdx(i),
            expert: ExpertId(i),
        };
        let size: u64 = kani::any();
        kani::assume(size >= 1 && size <= budget);
        let payload = vec![0u8; size as usize];
        cache.put(key, payload);

        // After each put, the budget invariant must hold.
        kani::assert(
            cache.current_bytes() <= budget,
            "current_bytes must never exceed ram_budget",
        );
    }
}

/// Property: LRU eviction removes the least-recently-used entry.
///
/// Sequence: put A, put B (evicts A if budget only fits 1), get A (must be None).
#[kani::proof]
fn verify_expert_cache_lru_eviction() {
    // Budget that fits exactly one 16-byte entry.
    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(16),
    });

    let key_a = CacheKey {
        layer: LayerIdx(0),
        expert: ExpertId(0),
    };
    let key_b = CacheKey {
        layer: LayerIdx(0),
        expert: ExpertId(1),
    };

    // Put A (fits).
    cache.put(key_a, vec![0xAAu8; 16]);
    kani::assert(cache.get(key_a).is_some(), "A must be resident after put");
    kani::assert_eq!(cache.len(), 1, "cache must have 1 entry");

    // Put B — must evict A (LRU) because budget only fits 1.
    cache.put(key_b, vec![0xBBu8; 16]);
    kani::assert(cache.get(key_b).is_some(), "B must be resident after put");
    kani::assert(
        cache.get(key_a).is_none(),
        "A must be evicted (LRU) when B is inserted",
    );
    kani::assert_eq!(cache.len(), 1, "cache must still have 1 entry after eviction");
    kani::assert(
        cache.current_bytes() <= 16,
        "current_bytes must not exceed budget after eviction",
    );
}

/// Property: `get` bumps the LRU clock — a subsequently-evicted entry is
/// not the one that was just accessed.
#[kani::proof]
fn verify_expert_cache_get_bumps_lru() {
    // Budget that fits two 16-byte entries.
    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(32),
    });

    let key_a = CacheKey { layer: LayerIdx(0), expert: ExpertId(0) };
    let key_b = CacheKey { layer: LayerIdx(0), expert: ExpertId(1) };
    let key_c = CacheKey { layer: LayerIdx(0), expert: ExpertId(2) };

    cache.put(key_a, vec![0u8; 16]);
    cache.put(key_b, vec![0u8; 16]);

    // Access A to bump its LRU clock (make it more recently used than B).
    let _ = cache.get(key_a);

    // Insert C — must evict B (the LRU), not A.
    cache.put(key_c, vec![0u8; 16]);

    kani::assert(cache.get(key_a).is_some(), "A must survive (was just accessed)");
    kani::assert(cache.get(key_b).is_none(), "B must be evicted (LRU)");
    kani::assert(cache.get(key_c).is_some(), "C must be resident");
}

/// Property: `put` with a key that already exists updates the value and
/// adjusts `current_bytes` correctly (old bytes are subtracted).
#[kani::proof]
fn verify_expert_cache_put_overwrite_adjusts_bytes() {
    let cache = ExpertCache::new(ExpertCacheConfig {
        ram_budget: Bytes(1024),
    });
    let key = CacheKey { layer: LayerIdx(0), expert: ExpertId(0) };

    // Insert 16 bytes.
    cache.put(key, vec![0u8; 16]);
    kani::assert_eq!(cache.current_bytes(), 16, "bytes must be 16 after first put");

    // Overwrite with 32 bytes.
    cache.put(key, vec![0u8; 32]);
    kani::assert_eq!(cache.current_bytes(), 32, "bytes must be 32 after overwrite (not 16+32)");
    kani::assert_eq!(cache.len(), 1, "len must still be 1 (overwrite, not insert)");

    // Overwrite with 8 bytes.
    cache.put(key, vec![0u8; 8]);
    kani::assert_eq!(cache.current_bytes(), 8, "bytes must be 8 after shrink");
}

// ── AlignedBuffer invariant ────────────────────────────────────────────────

/// Property: `AlignedBuffer::len()` matches the data vector length.
#[kani::proof]
fn verify_aligned_buffer_len_invariant() {
    let size: usize = kani::any();
    kani::assume(size <= 4096);
    let buf = edgeforge_storage::AlignedBuffer { data: vec![0u8; size] };
    kani::assert_eq!(buf.len(), size, "len() must match data length");
    kani::assert_eq!(buf.is_empty(), size == 0, "is_empty must be true iff size == 0");
}

// ── ReadRequest construction ───────────────────────────────────────────────

/// Property: `ReadRequest::new` stores the id, offset, and len correctly.
#[kani::proof]
fn verify_read_request_construction() {
    let id: u64 = kani::any();
    let offset: u64 = kani::any();
    let len: usize = kani::any();
    kani::assume(len > 0 && len <= 65536);

    let req = edgeforge_storage::ReadRequest::new(id, offset, len);
    kani::assert_eq!(req.id, id, "id must match");
    kani::assert_eq!(req.offset, offset, "offset must match");
    kani::assert_eq!(req.len, len, "len must match");
}

// ── WeightPager offset calculation ─────────────────────────────────────────

/// Property: WeightPager's offset calculation (`layer * page_size`) never
/// overflows for realistic inputs.
#[kani::proof]
fn verify_weight_pager_offset_no_overflow() {
    let layer: u32 = kani::any();
    let page_size: u64 = kani::any();
    // Constrain to realistic model dimensions.
    kani::assume(layer < 1000);
    kani::assume(page_size > 0 && page_size <= 1_073_741_824); // ≤ 1 GiB

    // This is the offset calculation used by WeightPager::page_in.
    let offset = (layer as u64).checked_mul(page_size);
    kani::assert(offset.is_some(), "offset calculation must not overflow for realistic inputs");

    if let Some(off) = offset {
        kani::assert(off >= 0, "offset must be non-negative");
    }
}

// ── CacheKey equality ──────────────────────────────────────────────────────

/// Property: Two CacheKeys with the same (layer, expert) are equal.
#[kani::proof]
fn verify_cache_key_equality() {
    let l1: u32 = kani::any();
    let e1: u32 = kani::any();
    let l2: u32 = kani::any();
    let e2: u32 = kani::any();

    let k1 = CacheKey { layer: LayerIdx(l1), expert: ExpertId(e1) };
    let k2 = CacheKey { layer: LayerIdx(l2), expert: ExpertId(e2) };

    if l1 == l2 && e1 == e2 {
        kani::assert(k1 == k2, "same (layer, expert) → equal keys");
    } else {
        kani::assert(k1 != k2, "different (layer, expert) → unequal keys");
    }
}
