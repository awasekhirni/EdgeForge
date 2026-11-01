// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! Paged KV cache (F-TIER-04, F-TIER-06).
//!
//! Phase 4: implements real paged KV cache with LRU eviction. Hot pages
//! (recent tokens) live in RAM; cold pages spill to SSD when the RAM budget
//! is exceeded.

use edgeforge_core::{Bytes, LayerIdx};
use parking_lot::Mutex;
use std::collections::HashMap;

/// Identifier for a KV cache page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KvPageId {
    pub layer: LayerIdx,
    pub page: u32,
}

/// A resident KV cache page.
pub struct KvPage {
    pub id: KvPageId,
    pub bytes: Vec<u8>,
}

/// Configuration for the KV pager.
#[derive(Clone, Debug)]
pub struct KvPagerConfig {
    /// RAM budget for resident KV pages.
    pub ram_budget: Bytes,
    /// Page size in bytes (typ. 4 MiB per F-TIER-06).
    pub page_size: Bytes,
    /// SSD directory for cold page spill (Phase 4: not yet implemented).
    pub ssd_dir: std::path::PathBuf,
}

/// The KV cache pager. Hot pages live in RAM; cold pages are evicted (LRU)
/// when the budget is exceeded.
///
/// Phase 4: implements RAM-only with LRU eviction. SSD spill is a Phase 5
/// deliverable.
pub struct KvCachePager {
    config: KvPagerConfig,
    inner: Mutex<Inner>,
}

struct Inner {
    /// Currently-resident pages, keyed by (layer, page).
    resident: HashMap<KvPageId, CacheEntry>,
    /// Current total bytes resident.
    current_bytes: u64,
    /// Monotonic clock for LRU.
    clock: u64,
    /// Total number of cache hits.
    hits: u64,
    /// Total number of cache misses.
    misses: u64,
    /// Total number of evictions.
    evictions: u64,
}

#[derive(Clone)]
struct CacheEntry {
    bytes: Vec<u8>,
    last_access: u64,
}

impl KvCachePager {
    #[must_use]
    pub fn new(config: KvPagerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(Inner {
                resident: HashMap::new(),
                current_bytes: 0,
                clock: 0,
                hits: 0,
                misses: 0,
                evictions: 0,
            }),
        }
    }

    /// Get a page from the cache. Returns `Some(bytes)` on hit, `None` on miss.
    ///
    /// On hit, the page's LRU clock is bumped.
    pub fn get(&self, id: KvPageId) -> Option<Vec<u8>> {
        let mut inner = self.inner.lock();
        inner.clock += 1;
        let clock = inner.clock;
        let entry = inner.resident.get_mut(&id)?;
        entry.last_access = clock;
        let bytes = entry.bytes.clone();
        inner.hits += 1;
        Some(bytes)
    }

    /// Put a page into the cache. If the RAM budget is exceeded, LRU pages
    /// are evicted until there is room.
    pub fn put(&self, id: KvPageId, bytes: Vec<u8>) {
        let mut inner = self.inner.lock();
        let incoming = bytes.len() as u64;

        // If the entry already exists, subtract its old size.
        if let Some(existing) = inner.resident.get(&id) {
            inner.current_bytes -= existing.bytes.len() as u64;
        }

        // Evict LRU pages until we have room.
        while inner.current_bytes + incoming > self.config.ram_budget.0 {
            let lru_key = match inner
                .resident
                .iter()
                .min_by_key(|(_, e)| e.last_access)
                .map(|(k, _)| *k)
            {
                Some(k) => k,
                None => break,
            };
            if let Some(v) = inner.resident.remove(&lru_key) {
                inner.current_bytes -= v.bytes.len() as u64;
                inner.evictions += 1;
            }
        }

        inner.clock += 1;
        let now = inner.clock;
        inner.resident.insert(
            id,
            CacheEntry {
                bytes,
                last_access: now,
            },
        );
        inner.current_bytes += incoming;
        inner.misses += 1;
    }

    /// Check if a page is resident without bumping its LRU clock.
    pub fn contains(&self, id: KvPageId) -> bool {
        self.inner.lock().resident.contains_key(&id)
    }

    /// Remove a specific page from the cache.
    pub fn evict(&self, id: KvPageId) -> bool {
        let mut inner = self.inner.lock();
        if let Some(v) = inner.resident.remove(&id) {
            inner.current_bytes -= v.bytes.len() as u64;
            inner.evictions += 1;
            true
        } else {
            false
        }
    }

    /// Spill a page to SSD (Phase 5: F-TIER-04 cold page spill).
    ///
    /// Writes the page's bytes to `{ssd_dir}/kv_{layer}_{page}.bin` and
    /// removes it from RAM. Returns the path the page was spilled to.
    ///
    /// # Errors
    /// Returns `Io` error if the SSD write fails.
    pub fn spill_to_ssd(
        &self,
        id: KvPageId,
    ) -> Result<std::path::PathBuf, edgeforge_core::EdgeForgeError> {
        let mut inner = self.inner.lock();
        let entry = inner.resident.remove(&id).ok_or_else(|| {
            edgeforge_core::EdgeForgeError::Engine(format!("page {:?} not resident", id))
        })?;
        inner.current_bytes -= entry.bytes.len() as u64;
        inner.evictions += 1;

        let path = self
            .config
            .ssd_dir
            .join(format!("kv_{}_{}.bin", id.layer.0, id.page));
        std::fs::write(&path, &entry.bytes).map_err(|e| edgeforge_core::EdgeForgeError::Io {
            path: path.clone(),
            source: e,
        })?;
        Ok(path)
    }

    /// Load a spilled page from SSD back into RAM (Phase 5).
    ///
    /// Reads the page from `{ssd_dir}/kv_{layer}_{page}.bin` and inserts
    /// it into the cache. Returns `true` if the page was loaded, `false`
    /// if no spilled page exists.
    ///
    /// # Errors
    /// Returns `Io` error if the SSD read fails.
    pub fn load_from_ssd(&self, id: KvPageId) -> Result<bool, edgeforge_core::EdgeForgeError> {
        let path = self
            .config
            .ssd_dir
            .join(format!("kv_{}_{}.bin", id.layer.0, id.page));
        if !path.exists() {
            return Ok(false);
        }
        let bytes = std::fs::read(&path).map_err(|e| edgeforge_core::EdgeForgeError::Io {
            path: path.clone(),
            source: e,
        })?;
        self.put(id, bytes);
        Ok(true)
    }

    /// Get a page, loading from SSD if not resident (Phase 5).
    ///
    /// 1. Check RAM cache → hit: return
    /// 2. Check SSD spill → load into RAM → return
    /// 3. Miss: return None
    pub fn get_or_load(&self, id: KvPageId) -> Option<Vec<u8>> {
        if let Some(bytes) = self.get(id) {
            return Some(bytes);
        }
        // Try loading from SSD spill.
        if self.load_from_ssd(id).ok()? {
            return self.get(id);
        }
        None
    }

    /// Current bytes resident in RAM.
    #[must_use]
    pub fn current_bytes(&self) -> u64 {
        self.inner.lock().current_bytes
    }

    /// Number of resident pages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().resident.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Cache hit rate in [0.0, 1.0].
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let inner = self.inner.lock();
        let total = inner.hits + inner.misses;
        if total == 0 {
            0.0
        } else {
            inner.hits as f64 / total as f64
        }
    }

    /// Total number of evictions since the pager was created.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.inner.lock().evictions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pager(budget: u64) -> KvCachePager {
        KvCachePager::new(KvPagerConfig {
            ram_budget: Bytes(budget),
            page_size: Bytes(4096),
            ssd_dir: std::path::PathBuf::from("/tmp"),
        })
    }

    fn id(layer: u32, page: u32) -> KvPageId {
        KvPageId {
            layer: LayerIdx(layer),
            page,
        }
    }

    #[test]
    fn put_and_get_hit() {
        let pager = make_pager(1024);
        pager.put(id(0, 0), vec![0xAB; 100]);
        assert_eq!(pager.get(id(0, 0)), Some(vec![0xAB; 100]));
        assert_eq!(pager.len(), 1);
        assert!(pager.contains(id(0, 0)));
    }

    #[test]
    fn miss_returns_none() {
        let pager = make_pager(1024);
        assert!(pager.get(id(0, 99)).is_none());
        assert!(!pager.contains(id(0, 99)));
    }

    #[test]
    fn lru_eviction_oldest_first() {
        // Budget fits 2 pages of 100 bytes each.
        let pager = make_pager(200);
        pager.put(id(0, 0), vec![0x01; 100]);
        pager.put(id(0, 1), vec![0x02; 100]);
        // Touch page 0 → page 1 becomes LRU.
        let _ = pager.get(id(0, 0));
        // Insert page 2 → evicts page 1 (LRU).
        pager.put(id(0, 2), vec![0x03; 100]);
        assert!(pager.get(id(0, 1)).is_none());
        assert!(pager.get(id(0, 0)).is_some());
        assert!(pager.get(id(0, 2)).is_some());
        assert_eq!(pager.evictions(), 1);
    }

    #[test]
    fn current_bytes_tracks_inserts() {
        let pager = make_pager(1024);
        pager.put(id(0, 0), vec![0; 100]);
        assert_eq!(pager.current_bytes(), 100);
        pager.put(id(0, 1), vec![0; 50]);
        assert_eq!(pager.current_bytes(), 150);
    }

    #[test]
    fn hit_rate_calculated_correctly() {
        let pager = make_pager(1024);
        // 1 miss (put), 1 hit (get)
        pager.put(id(0, 0), vec![0; 10]);
        let _ = pager.get(id(0, 0));
        assert!((pager.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn evict_specific_page() {
        let pager = make_pager(1024);
        pager.put(id(0, 0), vec![0; 100]);
        assert!(pager.evict(id(0, 0)));
        assert!(!pager.contains(id(0, 0)));
        assert_eq!(pager.current_bytes(), 0);
    }

    #[test]
    fn evict_nonexistent_returns_false() {
        let pager = make_pager(1024);
        assert!(!pager.evict(id(0, 0)));
    }

    #[test]
    fn put_replaces_existing() {
        let pager = make_pager(1024);
        pager.put(id(0, 0), vec![0x01; 100]);
        pager.put(id(0, 0), vec![0x02; 50]);
        assert_eq!(pager.current_bytes(), 50);
        assert_eq!(pager.get(id(0, 0)), Some(vec![0x02; 50]));
    }

    // ── Phase 5: SSD spill tests ──────────────────────────────────────

    fn make_pager_with_ssd(budget: u64) -> (KvCachePager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let pager = KvCachePager::new(KvPagerConfig {
            ram_budget: Bytes(budget),
            page_size: Bytes(4096),
            ssd_dir: dir.path().to_path_buf(),
        });
        (pager, dir)
    }

    #[test]
    fn spill_to_ssd_writes_file_and_evicts() {
        let (pager, _dir) = make_pager_with_ssd(1024);
        pager.put(id(0, 0), vec![0xAB; 100]);
        assert!(pager.contains(id(0, 0)));

        let path = pager.spill_to_ssd(id(0, 0)).unwrap();
        assert!(path.exists());
        assert!(
            !pager.contains(id(0, 0)),
            "page should be evicted from RAM after spill"
        );
        assert_eq!(pager.current_bytes(), 0);
    }

    #[test]
    fn load_from_ssd_restores_page() {
        let (pager, _dir) = make_pager_with_ssd(1024);
        pager.put(id(0, 0), vec![0xCD; 50]);
        pager.spill_to_ssd(id(0, 0)).unwrap();
        assert!(!pager.contains(id(0, 0)));

        let loaded = pager.load_from_ssd(id(0, 0)).unwrap();
        assert!(loaded);
        assert!(pager.contains(id(0, 0)));
        assert_eq!(pager.get(id(0, 0)), Some(vec![0xCD; 50]));
    }

    #[test]
    fn load_from_ssd_returns_false_for_missing() {
        let (pager, _dir) = make_pager_with_ssd(1024);
        let loaded = pager.load_from_ssd(id(0, 0)).unwrap();
        assert!(!loaded);
    }

    #[test]
    fn get_or_load_checks_ssd_spill() {
        let (pager, _dir) = make_pager_with_ssd(1024);
        pager.put(id(0, 0), vec![0xEF; 30]);
        pager.spill_to_ssd(id(0, 0)).unwrap();

        // get_or_load should find the page on SSD.
        let result = pager.get_or_load(id(0, 0));
        assert_eq!(result, Some(vec![0xEF; 30]));
    }

    #[test]
    fn get_or_load_returns_none_when_not_spilled() {
        let (pager, _dir) = make_pager_with_ssd(1024);
        assert!(pager.get_or_load(id(0, 99)).is_none());
    }

    #[test]
    fn spill_nonexistent_fails() {
        let (pager, _dir) = make_pager_with_ssd(1024);
        assert!(pager.spill_to_ssd(id(0, 0)).is_err());
    }
}
