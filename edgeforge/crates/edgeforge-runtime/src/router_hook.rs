// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! Router hook for SSD paging (F-SCHED-01).
//!
//! Phase 5: implements `GraphAwareRouterHook` that queries the streaming
//! graph for the lowest-latency expert source.

use edgeforge_core::{Celsius, ExpertId, LayerIdx, NodeId};
use edgeforge_graph::StreamingGraph;
use edgeforge_storage::ExpertCache;
use parking_lot::RwLock;
use std::sync::Arc;

/// What the runtime should do for a given (layer, expert).
#[derive(Clone, Copy, Debug)]
pub enum ExpertResolution {
    Ram,
    Ssd,
    Peer(NodeId),
    Surrogate,
}

/// The router hook trait. Called per (layer, expert) to decide the source.
pub trait RouterHook: Send + Sync {
    fn resolve_expert(&self, layer: LayerIdx, expert: ExpertId) -> ExpertResolution;
}

/// The graph-aware router hook (ADR-014, ADR-015).
///
/// Queries the streaming graph for the best source; falls back to SSD
/// if the graph is unhealthy.
pub struct GraphAwareRouterHook {
    pub cache: Arc<ExpertCache>,
    pub graph: Arc<dyn StreamingGraph>,
    pub thermal: Arc<RwLock<edgeforge_storage::ThermalState>>,
    pub ssd_fetch_time_us: u64,
    pub allow_surrogate: bool,
}

impl GraphAwareRouterHook {
    #[must_use]
    pub fn new(
        cache: Arc<ExpertCache>,
        graph: Arc<dyn StreamingGraph>,
        thermal: Arc<RwLock<edgeforge_storage::ThermalState>>,
        ssd_fetch_time_us: u64,
        allow_surrogate: bool,
    ) -> Self {
        Self {
            cache,
            graph,
            thermal,
            ssd_fetch_time_us,
            allow_surrogate,
        }
    }
}

impl RouterHook for GraphAwareRouterHook {
    fn resolve_expert(&self, layer: LayerIdx, expert: ExpertId) -> ExpertResolution {
        use edgeforge_storage::expert_cache::CacheKey;

        // 1. RAM hit — always wins.
        if self.cache.get(CacheKey { layer, expert }).is_some() {
            return ExpertResolution::Ram;
        }

        // 2. Graph query — only if healthy.
        if self.graph.healthy() {
            let thermal = self
                .thermal
                .try_read()
                .map(|g| g.current.unwrap_or(Celsius(50.0)))
                .unwrap_or(Celsius(50.0));
            if let Some(peer) = self.graph.best_peer_for_expert(layer, expert, thermal) {
                if peer.fetch_time_us < self.ssd_fetch_time_us {
                    return ExpertResolution::Peer(peer.peer);
                }
            }
        }

        // 3. Fallback to SSD (or surrogate if allowed).
        if self.allow_surrogate {
            ExpertResolution::Surrogate
        } else {
            ExpertResolution::Ssd
        }
    }
}

/// A simple router hook that always returns SSD (for testing/fallback).
pub struct SsdOnlyRouterHook;

impl RouterHook for SsdOnlyRouterHook {
    fn resolve_expert(&self, _layer: LayerIdx, _expert: ExpertId) -> ExpertResolution {
        ExpertResolution::Ssd
    }
}

/// A router hook that always returns RAM (for warm-cache testing).
pub struct RamOnlyRouterHook;

impl RouterHook for RamOnlyRouterHook {
    fn resolve_expert(&self, _layer: LayerIdx, _expert: ExpertId) -> ExpertResolution {
        ExpertResolution::Ram
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::NodeId;
    use edgeforge_storage::{ExpertCacheConfig, ThermalState};

    fn make_test_hook(allow_surrogate: bool) -> GraphAwareRouterHook {
        let cache = Arc::new(ExpertCache::new(ExpertCacheConfig {
            ram_budget: edgeforge_core::Bytes(1024 * 1024),
        }));

        // Create a stub graph that always returns None (unhealthy).
        struct UnhealthyGraph;
        #[async_trait::async_trait]
        impl StreamingGraph for UnhealthyGraph {
            fn best_peer_for_expert(
                &self,
                _: LayerIdx,
                _: ExpertId,
                _: Celsius,
            ) -> Option<edgeforge_graph::PeerEstimate> {
                None
            }
            fn estimate_fetch_time(&self, _: NodeId, _: ExpertId) -> u64 {
                0
            }
            fn predicted_expert_set(&self, _: u64, _: u32) -> Vec<(LayerIdx, ExpertId)> {
                Vec::new()
            }
            fn ingest(
                &self,
                _: edgeforge_core::ControlEvent,
            ) -> Result<(), edgeforge_graph::GraphError> {
                Ok(())
            }
            fn healthy(&self) -> bool {
                false
            }
        }

        let graph: Arc<dyn StreamingGraph> = Arc::new(UnhealthyGraph);
        let thermal = Arc::new(RwLock::new(ThermalState::default()));

        GraphAwareRouterHook::new(cache, graph, thermal, 10_000, allow_surrogate)
    }

    #[test]
    fn ssd_only_hook_returns_ssd() {
        let hook = SsdOnlyRouterHook;
        assert!(matches!(
            hook.resolve_expert(LayerIdx(0), ExpertId(1)),
            ExpertResolution::Ssd
        ));
    }

    #[test]
    fn ram_only_hook_returns_ram() {
        let hook = RamOnlyRouterHook;
        assert!(matches!(
            hook.resolve_expert(LayerIdx(0), ExpertId(1)),
            ExpertResolution::Ram
        ));
    }

    #[test]
    fn graph_aware_hook_falls_back_to_ssd_when_unhealthy() {
        let hook = make_test_hook(false);
        // Graph is unhealthy → should fall back to SSD.
        assert!(matches!(
            hook.resolve_expert(LayerIdx(0), ExpertId(1)),
            ExpertResolution::Ssd
        ));
    }

    #[test]
    fn graph_aware_hook_uses_surrogate_when_allowed() {
        let hook = make_test_hook(true);
        // Graph is unhealthy, surrogate allowed → should return Surrogate.
        assert!(matches!(
            hook.resolve_expert(LayerIdx(0), ExpertId(1)),
            ExpertResolution::Surrogate
        ));
    }

    #[test]
    fn graph_aware_hook_returns_ram_on_cache_hit() {
        let hook = make_test_hook(false);
        // Insert into cache first.
        use edgeforge_storage::expert_cache::CacheKey;
        hook.cache.put(
            CacheKey {
                layer: LayerIdx(0),
                expert: ExpertId(1),
            },
            vec![0xFF; 16],
        );
        // Should return Ram on hit.
        assert!(matches!(
            hook.resolve_expert(LayerIdx(0), ExpertId(1)),
            ExpertResolution::Ram
        ));
    }
}
