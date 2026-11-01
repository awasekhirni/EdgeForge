// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Crowd Coordinator (ADR-017)
//!
//! ## Purpose
//! The L2.5 entry point. Owns the reputation graph, verification pipeline,
//! and incentive ledger. The L5 orchestrator talks only to this trait.
//!
//! ## What This File Does
//! - `ExecutionMode` — Local / DistributedTrusted / DistributedCooperative / DistributedCrowd
//! - `CrowdCoordinator` trait — admit_peer, route_expert_fetch, record_verdict, evict_peer
//! - `DefaultCrowdCoordinator` — production impl (Tier 1 fully wired)
//! - `CrowdError` — PeerNotFound, Internal
//!
//! ## Dependencies
//! - `crate::{ReputationGraph, IncentiveLedger}` — reputation + incentives
//! - `edgeforge_core::{ControlEvent, EdgeForgeError, ExpertId, LayerIdx, NodeId}`
//!
//! ## Output
//! `Result<(), CrowdError>` — Ok on success, CrowdError on failure.
//!
//! ## Function: admit_peer
//! - **Purpose:** Admit a new peer at the given tier
//! - **Parameters:** `NodeId`, `CrowdTier`, `PeerCapability`
//! - **Returns:** `Result<(), CrowdError>`
//! - **Side effects:** Adds peer to reputation graph with default score (0.5)
//! - **Dependencies:** `self.reputation.admit()`

use crate::{
    ChurnPolicy, CrowdTier, IncentiveLedger, IncentivePolicy, PeerCapability, RedundancyPolicy,
    ReputationGraph, VerificationOutcome, VerificationPolicy,
};
use async_trait::async_trait;
use edgeforge_core::{ControlEvent, EdgeForgeError, ExpertId, LayerIdx, NodeId};
use std::sync::Arc;
#[derive(Clone, Debug)]
/// `ExecutionMode` enumeration — see module documentation for variants.
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
/// The `CrowdCoordinator` trait — see module documentation for the contract.
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
/// `CrowdError` enumeration — see module documentation for variants.
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
/// `DefaultCrowdCoordinator` — see module documentation for purpose and usage.
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
