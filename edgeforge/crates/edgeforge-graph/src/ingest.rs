// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
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
pub struct GraphIngestConfig {
    pub capacity: usize,
}
impl Default for GraphIngestConfig {
    fn default() -> Self {
        Self { capacity: 65_536 }
    }
}
/// `GraphIngest` — see module documentation for purpose and usage.
pub struct GraphIngest {
    pub(crate) core: Arc<GraphCore>,
    pub(crate) queue: Arc<ArrayQueue<ControlEvent>>,
}
impl GraphIngest {
    /// Creates a new instance with default configuration.
    pub fn new(core: Arc<GraphCore>, config: GraphIngestConfig) -> Self {
        Self {
            core,
            queue: Arc::new(ArrayQueue::new(config.capacity)),
        }
    }
    /// Tries to enqueue an event; drops if queue is full.
    pub fn try_enqueue(&self, event: ControlEvent) -> Result<(), GraphError> {
        match self.queue.push(event) {
            Ok(()) => {
                self.core
                    .events_ingested
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            }
            Err(_) => {
                self.core
                    .events_dropped
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err(GraphError::Dropped)
            }
        }
    }
    /// Drains the queue and applies events to the graph core.
    pub fn drain_and_apply(&self, now_ms: i64) -> usize {
        let mut n = 0;
        while let Some(event) = self.queue.pop() {
            self.apply_event(event, now_ms);
            n += 1;
        }
        n
    }
    fn apply_event(&self, event: ControlEvent, now_ms: i64) {
        use edgeforge_core::ControlEventKind::*;
        match event.kind {
            PeerHeartbeat {
                peer,
                rtt_ms,
                thermal_celsius,
                experts_resident,
            } => {
                self.core
                    .record_heartbeat(peer, rtt_ms, thermal_celsius, experts_resident, now_ms)
            }
            PeerLost { peer } => self.core.record_peer_lost(peer),
            RouterDecision {
                sequence,
                layer,
                experts,
            } => self
                .core
                .record_router_decision(sequence, layer, &experts, now_ms),
            _ => {}
        }
    }
}
