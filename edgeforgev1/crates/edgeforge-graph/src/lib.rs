// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
pub mod core_;
pub mod ingest;
pub mod query;
use async_trait::async_trait;
pub use core_::{GraphCore, PeerEstimate};
use edgeforge_core::{Celsius, ControlEvent, ExpertId, LayerIdx, NodeId};
pub use ingest::{GraphIngest, GraphIngestConfig};
pub use query::{GraphQuery, GraphQueryError};
#[async_trait]
pub trait StreamingGraph: Send + Sync + 'static {
    fn best_peer_for_expert(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
        thermal: Celsius,
    ) -> Option<PeerEstimate>;
    fn estimate_fetch_time(&self, peer: NodeId, expert: ExpertId) -> u64;
    fn predicted_expert_set(&self, sequence: u64, n: u32) -> Vec<(LayerIdx, ExpertId)>;
    fn ingest(&self, event: ControlEvent) -> Result<(), GraphError>;
    fn healthy(&self) -> bool;
}
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("dropped")]
    Dropped,
    #[error("unhealthy")]
    Unhealthy,
}
