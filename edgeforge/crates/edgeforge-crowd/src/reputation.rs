//! # Peer Reputation Graph (ADR-017)
//!
//! ## Purpose
//! Tracks per-peer success/failure counts, average latency (EMA), and quality
//! score in [0.0, 1.0]. The streaming graph reads this score when filtering
//! peer candidates.
//!
//! ## What This File Does
//! - `PeerReputation` — peer, tier, success_count, failure_count, avg_latency_ms, quality_score
//! - `ReputationGraph` — thread-safe hashmap of peer reputations
//! - `admit(peer, tier, now_ms)` — add a new peer with default score (0.5)
//! - `record_success(peer, latency, now_ms)` — update EMA latency + quality score
//! - `record_failure(peer, now_ms)` — decrement quality score
//! - `evict(peer)` — remove a peer
//! - `get(peer)` — snapshot of a peer's reputation
//!
//! ## Dependencies
//! - `parking_lot::RwLock` — thread-safe interior mutability
//! - `edgeforge_core::NodeId`
//! - `crate::CrowdTier`
//!
//! ## Output
//! `Option<PeerReputation>` — peer's reputation or None if not tracked.
//!
//! ## Quality Score Formula
//! `quality_score = 0.7 * success_rate + 0.3 * latency_score`
//! where `latency_score = (1.0 - (avg_latency_ms / 100.0).min(1.0)).max(0.0)`
//!
//! ## Function: record_success
//! - **Purpose:** Record a successful expert fetch and update quality score
//! - **Parameters:** `NodeId`, `latency_ms: f32`, `now_ms: i64`
//! - **Side effects:** Increments success_count, updates EMA avg_latency_ms,
//!   recalculates quality_score
//! - **Dependencies:** `PeerReputation::success_rate()`

use crate::CrowdTier;
use edgeforge_core::NodeId;
use parking_lot::RwLock;
use std::collections::HashMap;
#[derive(Clone, Debug)]
/// `PeerReputation` — see module documentation for purpose and usage.
pub struct PeerReputation { pub peer: NodeId, pub tier: CrowdTier, pub success_count: u64, pub failure_count: u64, pub avg_latency_ms: f32, pub quality_score: f32, pub last_updated_ms: i64 }
impl PeerReputation { pub fn success_rate(&self) -> f32 { let t = self.success_count + self.failure_count; if t == 0 { 0.5 } else { self.success_count as f32 / t as f32 } } }
/// `ReputationGraph` — see module documentation for purpose and usage.
pub struct ReputationGraph { inner: RwLock<HashMap<NodeId, PeerReputation>> }
impl ReputationGraph {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self { Self { inner: RwLock::new(HashMap::new()) } }
    /// Admits a new item into the collection.
    pub fn admit(&self, peer: NodeId, tier: CrowdTier, now_ms: i64) { self.inner.write().insert(peer, PeerReputation { peer, tier, success_count: 0, failure_count: 0, avg_latency_ms: 0.0, quality_score: 0.5, last_updated_ms: now_ms }); }
    /// Records a successful operation.
    pub fn record_success(&self, peer: NodeId, lat: f32, now_ms: i64) { if let Some(mut g) = self.inner.write().get_mut(&peer) { g.success_count += 1; g.avg_latency_ms = if g.avg_latency_ms == 0.0 { lat } else { 0.8 * g.avg_latency_ms + 0.2 * lat }; g.quality_score = 0.7 * g.success_rate() + 0.3 * (1.0 - (g.avg_latency_ms / 100.0).min(1.0)).max(0.0); g.last_updated_ms = now_ms; } }
    /// Records a failed operation.
    pub fn record_failure(&self, peer: NodeId, now_ms: i64) { if let Some(mut g) = self.inner.write().get_mut(&peer) { g.failure_count += 1; g.quality_score = 0.7 * g.success_rate() + 0.3 * g.quality_score; g.last_updated_ms = now_ms; } }
    /// Evicts an item from the collection.
    pub fn evict(&self, peer: NodeId) { self.inner.write().remove(&peer); }
    /// Retrieves an item from the cache.
    pub fn get(&self, peer: NodeId) -> Option<PeerReputation> { self.inner.read().get(&peer).cloned() }
    /// Returns the number of elements.
    pub fn len(&self) -> usize { self.inner.read().len() }
    /// Returns `true` if `empty` is true.
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}
impl Default for ReputationGraph { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn peer() -> NodeId { NodeId(Ulid::new()) }

    #[test]
    fn test_new_graph_empty() {
        let g = ReputationGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn test_admit_sets_default_score() {
        let g = ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        let rep = g.get(p).unwrap();
        assert_eq!(rep.quality_score, 0.5);
        assert_eq!(rep.success_count, 0);
        assert_eq!(rep.failure_count, 0);
        assert_eq!(rep.avg_latency_ms, 0.0);
    }

    #[test]
    fn test_admit_increases_len() {
        let g = ReputationGraph::new();
        g.admit(peer(), CrowdTier::Trusted, 0);
        assert_eq!(g.len(), 1);
        g.admit(peer(), CrowdTier::Cooperative, 0);
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn test_record_success_updates_score() {
        let g = ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        g.record_success(p, 10.0, 1);
        let rep = g.get(p).unwrap();
        assert_eq!(rep.success_count, 1);
        assert!(rep.quality_score > 0.5, "success should increase score");
        assert_eq!(rep.avg_latency_ms, 10.0);
    }

    #[test]
    fn test_record_success_ema_latency() {
        let g = ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        g.record_success(p, 10.0, 1);
        g.record_success(p, 20.0, 2);
        let rep = g.get(p).unwrap();
        // EMA: first=10, then 0.8*10 + 0.2*20 = 12
        assert!((rep.avg_latency_ms - 12.0).abs() < 0.01);
    }

    #[test]
    fn test_record_failure_decreases_score() {
        let g = ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Open, 0);
        g.record_failure(p, 1);
        g.record_failure(p, 2);
        let rep = g.get(p).unwrap();
        assert_eq!(rep.failure_count, 2);
        assert!(rep.quality_score < 0.5, "failures should decrease score");
    }

    #[test]
    fn test_success_rate_no_data() {
        let rep = PeerReputation {
            peer: peer(), tier: CrowdTier::Trusted, success_count: 0,
            failure_count: 0, avg_latency_ms: 0.0, quality_score: 0.5, last_updated_ms: 0,
        };
        assert_eq!(rep.success_rate(), 0.5);
    }

    #[test]
    fn test_success_rate_all_success() {
        let rep = PeerReputation {
            peer: peer(), tier: CrowdTier::Trusted, success_count: 10,
            failure_count: 0, avg_latency_ms: 0.0, quality_score: 0.5, last_updated_ms: 0,
        };
        assert_eq!(rep.success_rate(), 1.0);
    }

    #[test]
    fn test_success_rate_mixed() {
        let rep = PeerReputation {
            peer: peer(), tier: CrowdTier::Trusted, success_count: 7,
            failure_count: 3, avg_latency_ms: 0.0, quality_score: 0.5, last_updated_ms: 0,
        };
        assert!((rep.success_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_evict_removes_peer() {
        let g = ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        assert_eq!(g.len(), 1);
        g.evict(p);
        assert_eq!(g.len(), 0);
        assert!(g.get(p).is_none());
    }

    #[test]
    fn test_get_unknown_peer_returns_none() {
        let g = ReputationGraph::new();
        assert!(g.get(peer()).is_none());
    }

    #[test]
    fn test_record_success_on_unknown_peer_is_noop() {
        let g = ReputationGraph::new();
        g.record_success(peer(), 10.0, 0);
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn test_record_failure_on_unknown_peer_is_noop() {
        let g = ReputationGraph::new();
        g.record_failure(peer(), 0);
        assert_eq!(g.len(), 0);
    }
}
