// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-crowd — L2.5 Crowd Coordination (ADR-017)
//!
//! ## Purpose
//! Three-tier distribution model: Trusted Mesh (≤16 peers), Cooperative Pool
//! (≤100 peers, PoQ), Open Crowd (unbounded, optimistic verification).
//!
//! ## Modules
//! - `coordinator` — CrowdCoordinator trait + DefaultCrowdCoordinator + ExecutionMode
//! - `verification` — VerificationPolicy, samplers, outcomes, PeerCapability
//! - `reputation` — PeerReputation + ReputationGraph (EMA latency, quality score)
//! - `ledger` — IncentiveLedger (Tier 2 credit / Tier 3 token)
//!
//! ## Dependencies
//! - `edgeforge-core` — NodeId, ExpertId, LayerIdx, ControlEvent, EdgeForgeError
//! - `edgeforge-net` — mesh transport
//! - `edgeforge-security` — sandbox, crypto
//! - `parking_lot` — thread-safe reputation/ledger
//!
//! ## Architectural Invariants
//! 1. Tier 3 is opt-in only; no code path may auto-enable it.
//! 2. Tier 3 peers never see raw prompts (R-20).
//! 3. Verification never blocks beyond 50 ms (ADR-018, R-21).

pub mod churn;
pub mod coordinator;
pub mod ledger;
pub mod reputation;
pub mod verification;
pub use churn::{ChurnPolicy as ChurnPolicyStruct, ChurnResilientScheduler};
pub use coordinator::{CrowdCoordinator, CrowdError, DefaultCrowdCoordinator, ExecutionMode};
pub use ledger::{IncentiveLedger, IncentivePolicy, LedgerKind};
pub use reputation::{PeerReputation, ReputationGraph};
pub use verification::{
    ChurnPolicy, CrowdTier, PeerCapability, RedundancyPolicy, VerificationOutcome,
    VerificationPolicy, VerificationSampler,
};
