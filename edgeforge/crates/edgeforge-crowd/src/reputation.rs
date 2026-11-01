// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::CrowdTier;
use edgeforge_core::NodeId;
use parking_lot::RwLock;
use std::collections::HashMap;
#[derive(Clone, Debug)]
pub struct PeerReputation {
    pub peer: NodeId,
    pub tier: CrowdTier,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_latency_ms: f32,
    pub quality_score: f32,
    pub last_updated_ms: i64,
}
impl PeerReputation {
    pub fn success_rate(&self) -> f32 {
        let t = self.success_count + self.failure_count;
        if t == 0 {
            0.5
        } else {
            self.success_count as f32 / t as f32
        }
    }
}
pub struct ReputationGraph {
    inner: RwLock<HashMap<NodeId, PeerReputation>>,
}
impl ReputationGraph {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
    pub fn admit(&self, peer: NodeId, tier: CrowdTier, now_ms: i64) {
        self.inner.write().insert(
            peer,
            PeerReputation {
                peer,
                tier,
                success_count: 0,
                failure_count: 0,
                avg_latency_ms: 0.0,
                quality_score: 0.5,
                last_updated_ms: now_ms,
            },
        );
    }
    pub fn record_success(&self, peer: NodeId, lat: f32, now_ms: i64) {
        if let Some(mut g) = self.inner.write().get_mut(&peer) {
            g.success_count += 1;
            g.avg_latency_ms = if g.avg_latency_ms == 0.0 {
                lat
            } else {
                0.8 * g.avg_latency_ms + 0.2 * lat
            };
            g.quality_score =
                0.7 * g.success_rate() + 0.3 * (1.0 - (g.avg_latency_ms / 100.0).min(1.0)).max(0.0);
            g.last_updated_ms = now_ms;
        }
    }
    pub fn record_failure(&self, peer: NodeId, now_ms: i64) {
        if let Some(mut g) = self.inner.write().get_mut(&peer) {
            g.failure_count += 1;
            g.last_updated_ms = now_ms;
        }
    }
    pub fn evict(&self, peer: NodeId) {
        self.inner.write().remove(&peer);
    }
    pub fn get(&self, peer: NodeId) -> Option<PeerReputation> {
        self.inner.read().get(&peer).cloned()
    }
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
impl Default for ReputationGraph {
    fn default() -> Self {
        Self::new()
    }
}
