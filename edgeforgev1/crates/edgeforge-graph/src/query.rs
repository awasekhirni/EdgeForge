// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::core_::GraphCore;
use crate::{GraphError, PeerEstimate, StreamingGraph};
use async_trait::async_trait;
use edgeforge_core::{Celsius, ControlEvent, ExpertId, LayerIdx, NodeId};
use std::sync::Arc;
#[derive(Debug, thiserror::Error)]
pub enum GraphQueryError {
    #[error("unhealthy")]
    Unhealthy,
}
pub struct GraphQuery {
    pub(crate) core: Arc<GraphCore>,
}
impl GraphQuery {
    pub fn new(core: Arc<GraphCore>) -> Self {
        Self { core }
    }
}
#[async_trait]
impl StreamingGraph for GraphQuery {
    fn best_peer_for_expert(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
        thermal: Celsius,
    ) -> Option<PeerEstimate> {
        self.core.best_peer_for_expert(layer, expert, thermal)
    }
    fn estimate_fetch_time(&self, peer: NodeId, expert: ExpertId) -> u64 {
        self.core.estimate_fetch_time(peer, expert)
    }
    fn predicted_expert_set(&self, sequence: u64, n: u32) -> Vec<(LayerIdx, ExpertId)> {
        let hist = self.core.history.read();
        match hist.get(&sequence) {
            Some(entries) => entries
                .iter()
                .rev()
                .take(n as usize)
                .map(|&(l, e, _)| (l, e))
                .collect(),
            None => Vec::new(),
        }
    }
    fn ingest(&self, _event: ControlEvent) -> Result<(), GraphError> {
        Err(GraphError::Unhealthy)
    }
    fn healthy(&self) -> bool {
        self.core.is_healthy()
    }
}
