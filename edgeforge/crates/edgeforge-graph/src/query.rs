//! # Query Engine (ADR-014)
//!
//! ## Purpose
//! Read-only query layer over GraphCore. The <10 µs p99 query contract is
//! validated by the `query_latency` benchmark.
//!
//! ## What This File Does
//! - `GraphQuery` — production StreamingGraph implementation
//! - `GraphQueryError` — Unhealthy
//! - Implements all StreamingGraph trait methods
//! - `predicted_expert_set()` — H3 prediction from sequence history
//!
//! ## Dependencies
//! - `crate::core_::GraphCore` — the graph store
//! - `crate::{StreamingGraph, PeerEstimate, GraphError}`
//! - `edgeforge_core::{Celsius, ControlEvent, ExpertId, LayerIdx, NodeId}`
//!
//! ## Output
//! Query results: `Option<PeerEstimate>`, `Vec<(LayerIdx, ExpertId)>`, `u64`, `bool`.
//!
//! ## Function: predicted_expert_set
//! - **Purpose:** H3 speculation — predict experts for the next N tokens
//! - **Parameters:** `sequence: u64`, `n: u32` — sequence ID and prediction count
//! - **Returns:** `Vec<(LayerIdx, ExpertId)>` — predicted experts (most recent first)
//! - **Dependencies:** `self.core.history` — sequence routing history
//! - **Algorithm:** Returns the last N (layer, expert) pairs from the sequence's
//!   routing history, most recent first

use crate::core_::GraphCore;
use crate::{GraphError, PeerEstimate, StreamingGraph};
use async_trait::async_trait;
use edgeforge_core::{Celsius, ControlEvent, ExpertId, LayerIdx, NodeId};
use std::sync::Arc;
#[derive(Debug, thiserror::Error)]
/// `GraphQueryError` enumeration — see module documentation for variants.
pub enum GraphQueryError { #[error("unhealthy")] Unhealthy }
/// `GraphQuery` — see module documentation for purpose and usage.
pub struct GraphQuery { pub(crate) core: Arc<GraphCore> }
impl GraphQuery { pub fn new(core: Arc<GraphCore>) -> Self { Self { core } } }
#[async_trait]
impl StreamingGraph for GraphQuery {
    fn best_peer_for_expert(&self, layer: LayerIdx, expert: ExpertId, thermal: Celsius) -> Option<PeerEstimate> { self.core.best_peer_for_expert(layer, expert, thermal) }
    fn estimate_fetch_time(&self, peer: NodeId, expert: ExpertId) -> u64 { self.core.estimate_fetch_time(peer, expert) }
    fn predicted_expert_set(&self, sequence: u64, n: u32) -> Vec<(LayerIdx, ExpertId)> {
        let hist = self.core.history.read();
        match hist.get(&sequence) { Some(entries) => entries.iter().rev().take(n as usize).map(|&(l, e, _)| (l, e)).collect(), None => Vec::new() }
    }
    fn ingest(&self, _event: ControlEvent) -> Result<(), GraphError> { Err(GraphError::Unhealthy) }
    fn healthy(&self) -> bool { self.core.is_healthy() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn node() -> NodeId { NodeId(Ulid::new()) }

    #[test]
    fn test_query_healthy_returns_true() {
        let core = Arc::new(GraphCore::new());
        let q = GraphQuery::new(core);
        assert!(q.healthy());
    }

    #[test]
    fn test_query_unhealthy_returns_false() {
        let core = Arc::new(GraphCore::new());
        core.set_healthy(false);
        let q = GraphQuery::new(core);
        assert!(!q.healthy());
    }

    #[test]
    fn test_query_best_peer_delegates_to_core() {
        let core = Arc::new(GraphCore::new());
        let p = node();
        core.record_heartbeat(p, 10, 45.0, 5, 0);
        let q = GraphQuery::new(core);
        let r = q.best_peer_for_expert(LayerIdx(0), ExpertId(0), Celsius(50.0));
        assert!(r.is_some());
        assert_eq!(r.unwrap().peer, p);
    }

    #[test]
    fn test_query_estimate_fetch_time_delegates() {
        let core = Arc::new(GraphCore::new());
        let p = node();
        core.record_heartbeat(p, 25, 45.0, 5, 0);
        let q = GraphQuery::new(core);
        assert_eq!(q.estimate_fetch_time(p, ExpertId(0)), 25_000);
    }

    #[test]
    fn test_predicted_expert_set_empty_history() {
        let core = Arc::new(GraphCore::new());
        let q = GraphQuery::new(core);
        let result = q.predicted_expert_set(999, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_predicted_expert_set_returns_history() {
        let core = Arc::new(GraphCore::new());
        core.record_router_decision(1, LayerIdx(0), &[ExpertId(0), ExpertId(1), ExpertId(2)], 100);
        let q = GraphQuery::new(core);
        let result = q.predicted_expert_set(1, 10);
        assert_eq!(result.len(), 3);
        // Reversed order (most recent first)
        assert_eq!(result[0], (LayerIdx(0), ExpertId(2)));
    }

    #[test]
    fn test_predicted_expert_set_respects_n() {
        let core = Arc::new(GraphCore::new());
        core.record_router_decision(1, LayerIdx(0), &[ExpertId(0), ExpertId(1), ExpertId(2)], 100);
        let q = GraphQuery::new(core);
        let result = q.predicted_expert_set(1, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_ingest_returns_unhealthy_error() {
        let core = Arc::new(GraphCore::new());
        let q = GraphQuery::new(core);
        let event = ControlEvent {
            timestamp_ms: 0,
            kind: edgeforge_core::ControlEventKind::PeerLost { peer: node() },
        };
        assert!(q.ingest(event).is_err());
    }
}
