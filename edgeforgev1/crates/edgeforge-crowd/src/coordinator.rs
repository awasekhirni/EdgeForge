// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::{
    ChurnPolicy, CrowdTier, IncentiveLedger, IncentivePolicy, PeerCapability, RedundancyPolicy,
    ReputationGraph, VerificationOutcome, VerificationPolicy,
};
use async_trait::async_trait;
use edgeforge_core::{ControlEvent, EdgeForgeError, ExpertId, LayerIdx, NodeId};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub enum ExecutionMode {
    Local,
    DistributedTrusted,
    DistributedCooperative {
        verification: VerificationPolicy,
        redundancy: RedundancyPolicy,
    },
    DistributedCrowd {
        verification: VerificationPolicy,
        incentive: IncentivePolicy,
        churn: ChurnPolicy,
    },
}
#[async_trait]
pub trait CrowdCoordinator: Send + Sync + 'static {
    async fn admit_peer(
        &self,
        peer: NodeId,
        tier: CrowdTier,
        capability: PeerCapability,
    ) -> Result<(), CrowdError>;
    async fn route_expert_fetch(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
        tier: CrowdTier,
    ) -> Result<Vec<NodeId>, CrowdError>;
    async fn record_verdict(&self, event: ControlEvent) -> Result<(), CrowdError>;
    async fn evict_peer(&self, peer: NodeId) -> Result<(), CrowdError>;
    fn execution_mode(&self) -> &ExecutionMode;
}
#[derive(Debug, thiserror::Error)]
pub enum CrowdError {
    #[error("peer not found: {0:?}")]
    PeerNotFound(NodeId),
    #[error("internal: {0}")]
    Internal(String),
}
impl From<CrowdError> for EdgeForgeError {
    fn from(e: CrowdError) -> Self {
        EdgeForgeError::Mesh(e.to_string())
    }
}
pub struct DefaultCrowdCoordinator {
    pub reputation: Arc<ReputationGraph>,
    pub ledger: Arc<IncentiveLedger>,
    pub execution_mode: ExecutionMode,
}
impl DefaultCrowdCoordinator {
    pub fn new(execution_mode: ExecutionMode) -> Self {
        Self {
            reputation: Arc::new(ReputationGraph::new()),
            ledger: Arc::new(IncentiveLedger::new()),
            execution_mode,
        }
    }
}
#[async_trait]
impl CrowdCoordinator for DefaultCrowdCoordinator {
    async fn admit_peer(
        &self,
        peer: NodeId,
        _tier: CrowdTier,
        _cap: PeerCapability,
    ) -> Result<(), CrowdError> {
        self.reputation.admit(peer, CrowdTier::Trusted, 0);
        Ok(())
    }
    async fn route_expert_fetch(
        &self,
        _l: LayerIdx,
        _e: ExpertId,
        _t: CrowdTier,
    ) -> Result<Vec<NodeId>, CrowdError> {
        Ok(Vec::new())
    }
    async fn record_verdict(&self, _e: ControlEvent) -> Result<(), CrowdError> {
        Ok(())
    }
    async fn evict_peer(&self, peer: NodeId) -> Result<(), CrowdError> {
        self.reputation.evict(peer);
        Ok(())
    }
    fn execution_mode(&self) -> &ExecutionMode {
        &self.execution_mode
    }
}
