#![allow(unused_imports, unused_variables)]
//! # RAM-Resident LRU Expert Cache (F-TIER-01)
//!
//! ## Purpose
//! Holds the most recently used expert weight tensors in RAM so the weight
//! pager can skip SSD reads for hot experts. Eviction is LRU; experts are
//! read-only so eviction is a pure drop (no write-back).
//!
//! ## What This File Does
//! - `ExpertCacheConfig` — ram_budget
//! - `CacheKey` — (LayerIdx, ExpertId)
//! - `ExpertCache` — thread-safe LRU cache using parking_lot::Mutex
//! - `get(key)` — look up expert, bump LRU clock on hit
//! - `put(key, bytes)` — insert, evict LRU until budget fits
//! - `current_bytes()` — current RAM usage
//! - `len()` / `is_empty()` — entry count
//!
//! ## Dependencies
//! - `parking_lot::Mutex` — thread-safe interior mutability
//! - `edgeforge_core::{Bytes, ExpertId, LayerIdx}`
//!
//! ## Output
//! `Option<Vec<u8>>` — Some on cache hit, None on miss.
//!
//! ## Function: put
//! - **Purpose:** Insert an expert into the cache, evicting LRU entries if needed
//! - **Parameters:** `CacheKey` (layer + expert), `Vec<u8>` (weight bytes)
//! - **Side effects:** May evict 0..N LRU entries to fit the budget
//! - **Dependencies:** `Self::pick_lru()` for eviction selection
//! - **LRU clock:** Monotonic counter incremented on each get/put

use edgeforge_core::{Bytes, ExpertId, LayerIdx};
use parking_lot::Mutex;
use std::collections::HashMap;
#[derive(Clone, Debug)]
/// `ExpertCacheConfig` — see module documentation for purpose and usage.
pub struct ExpertCacheConfig { pub ram_budget: Bytes }
impl Default for ExpertCacheConfig { fn default() -> Self { Self { ram_budget: Bytes(1024 * 1024 * 1024) } } }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// `CacheKey` — see module documentation for purpose and usage.
pub struct CacheKey { pub layer: LayerIdx, pub expert: ExpertId }
/// `ExpertCache` — see module documentation for purpose and usage.
pub struct ExpertCache { inner: Mutex<Inner>, config: ExpertCacheConfig }
struct Inner { entries: HashMap<CacheKey, CacheEntry>, clock: u64, current_bytes: u64 }
#[derive(Clone)] struct CacheEntry { bytes: Vec<u8>, last_access: u64 }
impl ExpertCache {
    /// Creates a new instance with default configuration.
    pub fn new(config: ExpertCacheConfig) -> Self { Self { inner: Mutex::new(Inner { entries: HashMap::new(), clock: 0, current_bytes: 0 }), config } }
    /// Retrieves an item from the cache.
    pub fn get(&self, key: CacheKey) -> Option<Vec<u8>> {
        let mut inner = self.inner.lock();
        inner.clock += 1;
        let now = inner.clock;
        let clock = inner.clock;
        let entry = inner.entries.get_mut(&key)?;
        entry.last_access = clock;
        Some(entry.bytes.clone())
    }
    /// Inserts an item into the cache.
    pub fn put(&self, key: CacheKey, bytes: Vec<u8>) {
        let mut inner = self.inner.lock();
        let incoming = bytes.len() as u64;
        if let Some(existing) = inner.entries.get(&key) { inner.current_bytes -= existing.bytes.len() as u64; }
        while inner.current_bytes + incoming > self.config.ram_budget.0 {
            let lru_key = match inner.entries.iter().min_by_key(|(_, e)| e.last_access).map(|(k, _)| *k) { Some(k) => k, None => break };
            if let Some(v) = inner.entries.remove(&lru_key) { inner.current_bytes -= v.bytes.len() as u64; }
        }
        inner.clock += 1;
        let now = inner.clock;
        inner.entries.insert(key, CacheEntry { bytes, last_access: now });
        inner.current_bytes += incoming;
    }
    /// Returns the current bytes resident in RAM.
    pub fn current_bytes(&self) -> u64 { self.inner.lock().current_bytes }
    /// Returns the number of elements.
    pub fn len(&self) -> usize { self.inner.lock().entries.len() }
    /// Returns `true` if `empty` is true.
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(l: u32, e: u32) -> CacheKey { CacheKey { layer: LayerIdx(l), expert: ExpertId(e) } }
    #[test] fn put_and_get() {
        let c = ExpertCache::new(ExpertCacheConfig { ram_budget: Bytes(1024) });
        c.put(key(0, 1), vec![0xAB; 16]);
        assert_eq!(c.get(key(0, 1)), Some(vec![0xAB; 16]));
    }
    #[test] fn lru_eviction() {
        let c = ExpertCache::new(ExpertCacheConfig { ram_budget: Bytes(32) });
        c.put(key(0, 1), vec![0x01; 16]); c.put(key(0, 2), vec![0x02; 16]);
        let _ = c.get(key(0, 1)); c.put(key(0, 3), vec![0x03; 16]);
        assert!(c.get(key(0, 2)).is_none());
    }
}
