//! # Bounded-Channel Ingest (R-18 Load Shedding)
//!
//! ## Purpose
//! Receives ControlEvents via a bounded `crossbeam` array queue. When the
//! queue is full, oldest events are dropped (load shedding) to prevent the
//! graph from becoming a bottleneck.
//!
//! ## What This File Does
//! - `GraphIngestConfig` — capacity (default 65,536)
//! - `GraphIngest` — owns the queue + Arc<GraphCore>
//! - `try_enqueue(event)` — enqueue or drop (returns GraphError::Dropped)
//! - `drain_and_apply(now_ms)` — drain queue and apply events to GraphCore
//!
//! ## Dependencies
//! - `crossbeam_queue::ArrayQueue` — lock-free bounded queue
//! - `edgeforge_core::ControlEvent` — the event type
//! - `crate::core_::GraphCore` — the graph store
//!
//! ## Output
//! `Result<(), GraphError>` — Ok if enqueued, Dropped if queue full.
//!
//! ## Function: drain_and_apply
//! - **Purpose:** Drain all queued events and apply them to the graph core
//! - **Parameters:** `now_ms: i64` — current timestamp for heartbeat updates
//! - **Returns:** `usize` — number of events processed
//! - **Dependencies:** Calls `apply_event()` for each event, which dispatches
//!   to `record_heartbeat`, `record_peer_lost`, or `record_router_decision`

use crate::core_::GraphCore;
use crate::{GraphError, StreamingGraph};
use crossbeam_queue::ArrayQueue;
use edgeforge_core::ControlEvent;
use std::sync::Arc;
#[derive(Clone, Debug)]
/// `GraphIngestConfig` — see module documentation for purpose and usage.
pub struct GraphIngestConfig { pub capacity: usize }
impl Default for GraphIngestConfig { fn default() -> Self { Self { capacity: 65_536 } } }
/// `GraphIngest` — see module documentation for purpose and usage.
pub struct GraphIngest { pub(crate) core: Arc<GraphCore>, pub(crate) queue: Arc<ArrayQueue<ControlEvent>> }
impl GraphIngest {
    /// Creates a new instance with default configuration.
    pub fn new(core: Arc<GraphCore>, config: GraphIngestConfig) -> Self { Self { core, queue: Arc::new(ArrayQueue::new(config.capacity)) } }
    /// Tries to enqueue an event; drops if queue is full.
    pub fn try_enqueue(&self, event: ControlEvent) -> Result<(), GraphError> {
        match self.queue.push(event) { Ok(()) => { self.core.events_ingested.fetch_add(1, std::sync::atomic::Ordering::Relaxed); Ok(()) }
        Err(_) => { self.core.events_dropped.fetch_add(1, std::sync::atomic::Ordering::Relaxed); Err(GraphError::Dropped) } }
    }
    /// Drains the queue and applies events to the graph core.
    pub fn drain_and_apply(&self, now_ms: i64) -> usize {
        let mut n = 0;
        while let Some(event) = self.queue.pop() { self.apply_event(event, now_ms); n += 1; }
        n
    }
    fn apply_event(&self, event: ControlEvent, now_ms: i64) {
        use edgeforge_core::ControlEventKind::*;
        match event.kind {
            PeerHeartbeat { peer, rtt_ms, thermal_celsius, experts_resident } => self.core.record_heartbeat(peer, rtt_ms, thermal_celsius, experts_resident, now_ms),
            PeerLost { peer } => self.core.record_peer_lost(peer),
            RouterDecision { sequence, layer, experts } => self.core.record_router_decision(sequence, layer, &experts, now_ms),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::{ControlEventKind, NodeId};
    use ulid::Ulid;

    fn node() -> NodeId { NodeId(Ulid::new()) }

    fn make_event(kind: ControlEventKind) -> ControlEvent {
        ControlEvent { timestamp_ms: 0, kind }
    }

    #[test]
    fn test_try_enqueue_succeeds() {
        let core = Arc::new(GraphCore::new());
        let ingest = GraphIngest::new(core, GraphIngestConfig { capacity: 16 });
        let event = make_event(ControlEventKind::PeerLost { peer: node() });
        assert!(ingest.try_enqueue(event).is_ok());
    }

    #[test]
    fn test_try_enqueue_full_drops() {
        let core = Arc::new(GraphCore::new());
        let ingest = GraphIngest::new(core, GraphIngestConfig { capacity: 2 });
        ingest.try_enqueue(make_event(ControlEventKind::PeerLost { peer: node() })).unwrap();
        ingest.try_enqueue(make_event(ControlEventKind::PeerLost { peer: node() })).unwrap();
        let result = ingest.try_enqueue(make_event(ControlEventKind::PeerLost { peer: node() }));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::Dropped));
    }

    #[test]
    fn test_drain_and_apply_heartbeat() {
        let core = Arc::new(GraphCore::new());
        let ingest = GraphIngest::new(core.clone(), GraphIngestConfig { capacity: 16 });
        let p = node();
        ingest.try_enqueue(make_event(ControlEventKind::PeerHeartbeat {
            peer: p, rtt_ms: 10, thermal_celsius: 45.0, experts_resident: 5,
        })).unwrap();
        let n = ingest.drain_and_apply(100);
        assert_eq!(n, 1);
        assert_eq!(core.peers.len(), 1);
    }

    #[test]
    fn test_drain_and_apply_peer_lost() {
        let core = Arc::new(GraphCore::new());
        core.record_heartbeat(node(), 10, 45.0, 5, 0);
        let ingest = GraphIngest::new(core.clone(), GraphIngestConfig { capacity: 16 });
        let p = node();
        core.record_heartbeat(p, 10, 45.0, 5, 0);
        assert_eq!(core.peers.len(), 2);
        ingest.try_enqueue(make_event(ControlEventKind::PeerLost { peer: p })).unwrap();
        ingest.drain_and_apply(100);
        assert_eq!(core.peers.len(), 1);
    }

    #[test]
    fn test_drain_and_apply_router_decision() {
        let core = Arc::new(GraphCore::new());
        let ingest = GraphIngest::new(core.clone(), GraphIngestConfig { capacity: 16 });
        ingest.try_enqueue(make_event(ControlEventKind::RouterDecision {
            sequence: 1, layer: edgeforge_core::LayerIdx(0),
            experts: vec![edgeforge_core::ExpertId(0)],
        })).unwrap();
        ingest.drain_and_apply(100);
        let hist = core.history.read();
        assert!(hist.contains_key(&1));
    }

    #[test]
    fn test_drain_empty_returns_zero() {
        let core = Arc::new(GraphCore::new());
        let ingest = GraphIngest::new(core, GraphIngestConfig { capacity: 16 });
        assert_eq!(ingest.drain_and_apply(0), 0);
    }

    #[test]
    fn test_config_default_capacity() {
        assert_eq!(GraphIngestConfig::default().capacity, 65_536);
    }
}
