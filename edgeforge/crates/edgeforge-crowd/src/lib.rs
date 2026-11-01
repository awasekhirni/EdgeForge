#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-crowd — L2.5 Crowd Coordination (ADR-017)
//!
//! ## Purpose
//! Three-tier distribution model: Trusted Mesh (≤16 peers), Cooperative Pool
//! (≤100 peers, PoQ), Open Crowd (unbounded, optimistic verification).
//!
//! ## Modules
//! - `coordinator` — CrowdCoordinator trait + DefaultCrowdCoordinator + ExecutionMode
//! - `verification` — VerificationPolicy, PoQSampler, QuorumCalculator,
//!   VerdictAggregator, PeerVerdict, VerificationResult, VerificationOutcome
//! - `reputation` — PeerReputation + ReputationGraph (EMA latency, quality score)
//! - `ledger` — IncentiveLedger (Tier 2 credit / Tier 3 token)
//! - `churn` — ChurnResilientScheduler + ChurnPolicy
//!
//! ## Tier 2/3 Verification Pipeline
//! When `route_expert_fetch` is called for Tier 2 (Cooperative) or Tier 3 (Open):
//! 1. **Peer selection** — select peers that host the requested expert, sorted by quality score
//! 2. **PoQ sampling** — use `PoQSampler` to select a subset of peers as verifiers
//! 3. **Return** the selected fetch peer + verifier list
//!
//! When `record_verdict` is called with a `ControlEvent::VerificationVerdict`:
//! 1. **Store** the individual `PeerVerdict`
//! 2. **Update reputation** — record success/failure based on the verdict
//! 3. **Post to ledger** — credit/debit the peer's incentive balance
//!
//! When `aggregate_verdicts` is called:
//! 1. **Check quorum** — has enough verifiers responded?
//! 2. **Aggregate** — use `VerdictAggregator` to produce an aggregated `VerificationOutcome`
//! 3. **Clean up** — remove finalized jobs from the pending map
//!
//! ## Dependencies
//! - `edgeforge-core` — NodeId, ExpertId, LayerIdx, ControlEvent, EdgeForgeError
//! - `edgeforge-net` — mesh transport
//! - `edgeforge-security` — sandbox, crypto
//! - `edgeforge-observability` — FunctionLogger for call logging
//! - `parking_lot` — thread-safe reputation/ledger/peers/pending
//! - `chrono` — timestamps
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

pub use churn::{ChurnResilientScheduler, ChurnPolicy as ChurnPolicyStruct};
pub use coordinator::{CrowdCoordinator, CrowdError, DefaultCrowdCoordinator, ExecutionMode};
pub use ledger::{IncentiveLedger, IncentivePolicy, LedgerKind};
pub use reputation::{PeerReputation, ReputationGraph};
pub use verification::{
    CrowdTier, ChurnPolicy, PeerCapability, PeerVerdict, PoQSampler, QuorumCalculator,
    RedundancyPolicy, VerificationOutcome, VerificationPolicy, VerificationResult,
    VerificationSampler, VerdictAggregator,
};
