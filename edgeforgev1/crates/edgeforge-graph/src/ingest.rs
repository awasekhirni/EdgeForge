// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::core_::GraphCore;
use crate::{GraphError, StreamingGraph};
use crossbeam_queue::ArrayQueue;
use edgeforge_core::ControlEvent;
use std::sync::Arc;
#[derive(Clone, Debug)]
pub struct GraphIngestConfig {
    pub capacity: usize,
}
impl Default for GraphIngestConfig {
    fn default() -> Self {
        Self { capacity: 65_536 }
    }
}
pub struct GraphIngest {
    pub(crate) core: Arc<GraphCore>,
    pub(crate) queue: Arc<ArrayQueue<ControlEvent>>,
}
impl GraphIngest {
    pub fn new(core: Arc<GraphCore>, config: GraphIngestConfig) -> Self {
        Self {
            core,
            queue: Arc::new(ArrayQueue::new(config.capacity)),
        }
    }
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
