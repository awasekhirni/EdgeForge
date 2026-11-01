//! # Crowd Coordinator (ADR-017)
//!
//! ## Purpose
//! The L2.5 entry point. Owns the reputation graph, verification pipeline,
//! and incentive ledger. The L5 orchestrator talks only to this trait.
//!
//! ## What This File Does
//! - `ExecutionMode` — Local / DistributedTrusted / DistributedCooperative / DistributedCrowd
//! - `CrowdCoordinator` trait — admit_peer, route_expert_fetch, record_verdict, evict_peer,
//!   pending_verdicts, aggregate_verdicts
//! - `DefaultCrowdCoordinator` — production impl with real peer selection,
//!   PoQ sampling, quorum calculation, and verdict aggregation
//! - `CrowdError` — PeerNotFound, NoPeersAvailable, Internal
//!
//! ## Tier 2/3 Verification Pipeline
//! When `route_expert_fetch` is called for Tier 2 (Cooperative) or Tier 3 (Open):
//! 1. **Peer selection** — select peers that host the requested expert, sorted by quality score
//! 2. **PoQ sampling** — use `PoQSampler` to select a subset of peers as verifiers
//! 3. **Return** the selected fetch peer + verifier list
//!
//! When `record_verdict` is called with a `ControlEvent::VerificationVerdict`:
//! 1. **Store** the individual `PeerVerdict`
//! 2. **Check quorum** — has enough verifiers responded?
//! 3. **Aggregate** — use `VerdictAggregator` to produce an aggregated `VerificationOutcome`
//! 4. **Update reputation** — record success/failure based on the verdict
//! 5. **Post to ledger** — credit/debit the peer's incentive balance
//!
//! ## Dependencies
//! - `crate::{ReputationGraph, IncentiveLedger, PoQSampler, QuorumCalculator, VerdictAggregator, PeerVerdict, VerificationResult}`
//! - `edgeforge_core::{ControlEvent, ControlEventKind, EdgeForgeError, ExpertId, LayerIdx, NodeId}`
//!
//! ## Output
//! `Result<Vec<NodeId>, CrowdError>` from route_expert_fetch;
//! `Result<VerificationResult, CrowdError>` from aggregate_verdicts.

use crate::{
    IncentiveLedger, IncentivePolicy, PeerCapability, PeerVerdict, PoQSampler, QuorumCalculator,
    ReputationGraph, VerificationOutcome, VerificationPolicy, VerificationResult, VerdictAggregator,
    CrowdTier, ChurnPolicy, RedundancyPolicy,
};
use async_trait::async_trait;
use edgeforge_core::{ControlEvent, ControlEventKind, EdgeForgeError, ExpertId, LayerIdx, NodeId};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The execution mode — determines the crowd tier + verification policy.
#[derive(Clone, Debug)]
pub enum ExecutionMode {
    /// Tier 1 — local-only inference. No peers, no verification.
    Local,
    /// Tier 1 — distributed trusted mesh (≤16 peers). mTLS is sufficient.
    DistributedTrusted,
    /// Tier 2 — cooperative pool (≤100 peers). PoQ verification.
    DistributedCooperative {
        verification: VerificationPolicy,
        redundancy: RedundancyPolicy,
    },
    /// Tier 3 — open crowd (unbounded). Optimistic + eventual verification.
    DistributedCrowd {
        verification: VerificationPolicy,
        incentive: IncentivePolicy,
        churn: ChurnPolicy,
    },
}

impl ExecutionMode {
    /// Returns the crowd tier for this execution mode.
    #[must_use]
    pub fn tier(&self) -> CrowdTier {
        match self {
            Self::Local | Self::DistributedTrusted => CrowdTier::Trusted,
            Self::DistributedCooperative { .. } => CrowdTier::Cooperative,
            Self::DistributedCrowd { .. } => CrowdTier::Open,
        }
    }

    /// Returns the verification policy (if any).
    #[must_use]
    pub fn verification_policy(&self) -> Option<&VerificationPolicy> {
        match self {
            Self::DistributedCooperative { verification, .. } => Some(verification),
            Self::DistributedCrowd { verification, .. } => Some(verification),
            _ => None,
        }
    }
}

/// The crowd coordinator trait — the L2.5 entry point.
///
/// ## Contract
/// - `admit_peer()` — add a peer to the reputation graph at the given tier.
/// - `route_expert_fetch()` — select peers for an expert fetch + verifiers.
/// - `record_verdict()` — store a peer's verification verdict.
/// - `aggregate_verdicts()` — aggregate pending verdicts into a single outcome.
/// - `evict_peer()` — remove a peer from the mesh.
#[async_trait]
pub trait CrowdCoordinator: Send + Sync + 'static {
    /// Admit a new peer at the given tier.
    async fn admit_peer(&self, peer: NodeId, tier: CrowdTier, capability: PeerCapability) -> Result<(), CrowdError>;

    /// Route an expert fetch — select the fetch peer + PoQ verifiers.
    async fn route_expert_fetch(&self, layer: LayerIdx, expert: ExpertId, tier: CrowdTier) -> Result<Vec<NodeId>, CrowdError>;

    /// Record a peer's verification verdict.
    async fn record_verdict(&self, event: ControlEvent) -> Result<(), CrowdError>;

    /// Aggregate pending verdicts for a job into a single outcome.
    async fn aggregate_verdicts(&self, job: &edgeforge_core::JobId) -> Result<VerificationResult, CrowdError>;

    /// Evict a peer from the mesh.
    async fn evict_peer(&self, peer: NodeId) -> Result<(), CrowdError>;

    /// Returns the execution mode.
    fn execution_mode(&self) -> &ExecutionMode;
}

/// Crowd coordinator errors.
#[derive(Debug, thiserror::Error)]
pub enum CrowdError {
    /// The peer was not found in the reputation graph.
    #[error("peer not found: {0:?}")]
    PeerNotFound(NodeId),
    /// No peers are available to serve the request.
    #[error("no peers available for layer {0:?} expert {1:?}")]
    NoPeersAvailable(LayerIdx, ExpertId),
    /// Internal error.
    #[error("internal: {0}")]
    Internal(String),
}

impl From<CrowdError> for EdgeForgeError {
    fn from(e: CrowdError) -> Self {
        EdgeForgeError::Mesh(e.to_string())
    }
}

/// Peer metadata stored in the coordinator.
#[derive(Clone, Debug)]
struct PeerEntry {
    capability: PeerCapability,
    tier: CrowdTier,
}

/// Pending verdicts for a job, keyed by job ID.
#[derive(Clone, Debug, Default)]
struct PendingVerdicts {
    /// Individual peer verdicts received so far.
    verdicts: Vec<PeerVerdict>,
    /// The total number of verifiers expected (from PoQ sampling).
    expected_verifiers: usize,
    /// Whether the deadline has elapsed.
    deadline_elapsed: bool,
}

/// The production crowd coordinator with real peer selection, PoQ sampling,
/// quorum calculation, and verdict aggregation.
pub struct DefaultCrowdCoordinator {
    /// The reputation graph.
    pub reputation: Arc<ReputationGraph>,
    /// The incentive ledger.
    pub ledger: Arc<IncentiveLedger>,
    /// The execution mode (determines the tier + verification policy).
    pub execution_mode: ExecutionMode,
    /// Peer metadata (capability + tier), keyed by NodeId.
    peers: RwLock<HashMap<NodeId, PeerEntry>>,
    /// Pending verdicts, keyed by job ID.
    pending: RwLock<HashMap<edgeforge_core::JobId, PendingVerdicts>>,
}

impl DefaultCrowdCoordinator {
    /// Creates a new coordinator with the given execution mode.
    #[must_use]
    pub fn new(execution_mode: ExecutionMode) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "coordinator.rs",
            "DefaultCrowdCoordinator::new",
        );
        tracing::info!(
            tier = ?execution_mode.tier(),
            "DefaultCrowdCoordinator: initialized"
        );
        Self {
            reputation: Arc::new(ReputationGraph::new()),
            ledger: Arc::new(IncentiveLedger::new()),
            execution_mode,
            peers: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the number of admitted peers.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.read().len()
    }

    /// Returns the number of pending verification jobs.
    #[must_use]
    pub fn pending_job_count(&self) -> usize {
        self.pending.read().len()
    }

    /// Select peers that host the given (layer, expert) pair, sorted by quality score.
    fn select_hosting_peers(&self, layer: LayerIdx, expert: ExpertId) -> Vec<(NodeId, f32)> {
        let peers = self.peers.read();
        let mut candidates: Vec<(NodeId, f32)> = peers
            .iter()
            .filter(|(_, entry)| {
                entry
                    .capability
                    .hosted_experts
                    .contains(&(layer, expert))
            })
            .filter_map(|(peer, _)| {
                self.reputation.get(*peer).map(|rep| (*peer, rep.quality_score))
            })
            .collect();
        // Sort by quality score descending (highest first).
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }

    /// Returns all peers at the given tier with their quality scores.
    fn peers_at_tier(&self, tier: CrowdTier) -> Vec<(NodeId, f32)> {
        let peers = self.peers.read();
        peers
            .iter()
            .filter(|(_, entry)| entry.tier == tier)
            .filter_map(|(peer, _)| {
                self.reputation.get(*peer).map(|rep| (*peer, rep.quality_score))
            })
            .collect()
    }

    /// Returns the PoQ sample rate from the execution mode's verification policy.
    fn poq_sample_rate(&self) -> f32 {
        match self.execution_mode.verification_policy() {
            Some(VerificationPolicy {
                sampler: crate::VerificationSampler::ProofOfQuality { sample_rate },
                ..
            }) => *sample_rate,
            Some(VerificationPolicy {
                sampler: crate::VerificationSampler::Optimistic { recheck_rate },
                ..
            }) => *recheck_rate,
            _ => 0.0,
        }
    }
}

#[async_trait]
impl CrowdCoordinator for DefaultCrowdCoordinator {
    /// Admit a new peer at the given tier.
    ///
    /// ## Parameters
    /// - `peer`: The peer's node ID.
    /// - `tier`: The crowd tier to admit at.
    /// - `capability`: The peer's hardware capability + hosted experts.
    ///
    /// ## Returns
    /// `Ok(())` on success.
    ///
    /// ## Side Effects
    /// - Adds the peer to the reputation graph with default score (0.5)
    /// - Stores the peer's capability for later expert routing
    async fn admit_peer(&self, peer: NodeId, tier: CrowdTier, capability: PeerCapability) -> Result<(), CrowdError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "coordinator.rs",
            "admit_peer",
        );

        let now_ms = chrono::Utc::now().timestamp_millis();
        self.reputation.admit(peer, tier, now_ms);

        {
            let mut peers = self.peers.write();
            peers.insert(peer, PeerEntry { capability, tier });
        }

        tracing::info!(
            peer = %peer,
            tier = %tier,
            "admit_peer: peer admitted"
        );

        guard.success();
        Ok(())
    }

    /// Route an expert fetch — select the fetch peer + PoQ verifiers.
    ///
    /// ## Algorithm
    /// - Tier 1 (Trusted): return the best peer that hosts the expert (or all
    ///   peers if none host it specifically).
    /// - Tier 2 (Cooperative): return the best hosting peer + PoQ-sampled
    ///   verifiers from the same tier.
    /// - Tier 3 (Open): return k redundant peers (redundancy factor) + PoQ-sampled
    ///   verifiers.
    ///
    /// ## Parameters
    /// - `layer`: The layer index of the requested expert.
    /// - `expert`: The expert ID.
    /// - `tier`: The crowd tier to route within.
    ///
    /// ## Returns
    /// `Ok(Vec<NodeId>)` — the selected peers (fetch peer first, then verifiers).
    /// Returns `NoPeersAvailable` if no peers are admitted at the given tier.
    async fn route_expert_fetch(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
        tier: CrowdTier,
    ) -> Result<Vec<NodeId>, CrowdError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "coordinator.rs",
            "route_expert_fetch",
        );

        tracing::info!(
            layer = layer.0,
            expert = expert.0,
            tier = %tier,
            "route_expert_fetch: routing expert fetch"
        );

        // Get all peers at this tier.
        let tier_peers = self.peers_at_tier(tier);
        if tier_peers.is_empty() {
            let err = CrowdError::NoPeersAvailable(layer, expert);
            guard.error(&err.to_string());
            return Err(err);
        }

        // Select peers that host this expert.
        let hosting_peers = self.select_hosting_peers(layer, expert);

        let mut result: Vec<NodeId> = Vec::new();

        match tier {
            CrowdTier::Trusted => {
                // Tier 1: return the best hosting peer (or the best peer overall).
                if let Some((peer, _)) = hosting_peers.first() {
                    result.push(*peer);
                } else {
                    // No peer hosts this expert specifically — return the best peer.
                    result.push(tier_peers[0].0);
                }
            }
            CrowdTier::Cooperative | CrowdTier::Open => {
                // Tier 2/3: return the fetch peer + PoQ verifiers.

                // Select the fetch peer (best hosting peer, or best overall).
                let fetch_peer = hosting_peers
                    .first()
                    .map(|(p, _)| *p)
                    .unwrap_or(tier_peers[0].0);
                result.push(fetch_peer);

                // PoQ-sample verifiers from the tier (excluding the fetch peer).
                let verifier_candidates: Vec<(NodeId, f32)> = tier_peers
                    .iter()
                    .filter(|(p, _)| *p != fetch_peer)
                    .copied()
                    .collect();

                let sample_rate = self.poq_sample_rate();
                if sample_rate > 0.0 && !verifier_candidates.is_empty() {
                    let verifiers = PoQSampler::sample(&verifier_candidates, sample_rate);
                    result.extend(verifiers);
                }

                // For Tier 3, add redundant fetch peers (k-of-n).
                if tier == CrowdTier::Open {
                    let redundancy = match &self.execution_mode {
                        ExecutionMode::DistributedCrowd { .. } => {
                            // Default to 3 for Tier 3.
                            3u8
                        }
                        _ => 1,
                    };
                    for (peer, _) in tier_peers.iter().skip(1).take(redundancy.saturating_sub(1) as usize) {
                        if !result.contains(peer) {
                            result.push(*peer);
                        }
                    }
                }
            }
        }

        tracing::info!(
            selected_count = result.len(),
            fetch_peer = ?result.first(),
            "route_expert_fetch: peers selected"
        );

        guard.success();
        Ok(result)
    }

    /// Record a peer's verification verdict.
    ///
    /// ## Purpose
    /// Stores the individual `PeerVerdict` from a `ControlEvent::VerificationVerdict`.
    /// The verdict is added to the pending verdicts for the job. The caller can
    /// then call `aggregate_verdicts` to produce the final `VerificationOutcome`.
    ///
    /// ## Side Effects
    /// - Stores the verdict in the pending map
    /// - Updates the peer's reputation (success for Verified, failure for Corrected)
    /// - Posts to the incentive ledger (credit for Verified, debit for Corrected)
    ///
    /// ## Parameters
    /// - `event`: The `ControlEvent::VerificationVerdict` event.
    async fn record_verdict(&self, event: ControlEvent) -> Result<(), CrowdError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "coordinator.rs",
            "record_verdict",
        );

        // Only process VerificationVerdict events.
        let (job, peer, tier_core, outcome_core, latency_ms) = match &event.kind {
            ControlEventKind::VerificationVerdict {
                job,
                peer,
                tier,
                outcome,
                latency_ms,
            } => (*job, *peer, *tier, *outcome, *latency_ms),
            _ => {
                let msg = format!("record_verdict: expected VerificationVerdict, got {:?}", event.kind);
                guard.error(&msg);
                return Err(CrowdError::Internal(msg));
            }
        };

        // Convert edgeforge_core::VerificationOutcome → crate::VerificationOutcome.
        // Core has: Verified, Pending, Failed, Corrected
        // Crowd has: NotRequired, Verified, Pending, Corrected
        // Mapping: Failed → Corrected (a failed verification = correction needed)
        let outcome = match outcome_core {
            edgeforge_core::VerificationOutcome::Verified => VerificationOutcome::Verified,
            edgeforge_core::VerificationOutcome::Pending => VerificationOutcome::Pending,
            edgeforge_core::VerificationOutcome::Failed => VerificationOutcome::Corrected,
            edgeforge_core::VerificationOutcome::Corrected => VerificationOutcome::Corrected,
        };

        // Convert edgeforge_core::CrowdTier → crate::CrowdTier.
        let tier = match tier_core {
            edgeforge_core::CrowdTier::Trusted => CrowdTier::Trusted,
            edgeforge_core::CrowdTier::Cooperative => CrowdTier::Cooperative,
            edgeforge_core::CrowdTier::Open => CrowdTier::Open,
        };

        let now_ms = chrono::Utc::now().timestamp_millis();

        tracing::info!(
            job = %job,
            peer = %peer,
            tier = %tier,
            outcome = %outcome,
            latency_ms,
            "record_verdict: storing verdict"
        );

        // Store the verdict.
        {
            let mut pending = self.pending.write();
            let entry = pending.entry(job).or_default();
            entry.verdicts.push(PeerVerdict {
                peer,
                outcome,
                latency_ms,
                timestamp_ms: now_ms,
            });
        }

        // Update reputation based on the verdict.
        match outcome {
            VerificationOutcome::Verified => {
                self.reputation.record_success(peer, latency_ms as f32, now_ms);
                // Credit the peer.
                self.ledger.post(crate::ledger::LedgerEntry {
                    peer,
                    tier,
                    kind: crate::ledger::EntryKind::Credit,
                    amount: 1,
                    reason: "verification passed".to_string(),
                    related_job: Some(job.to_string()),
                    created_at_ms: now_ms,
                });
            }
            VerificationOutcome::Corrected => {
                self.reputation.record_failure(peer, now_ms);
                // Debit the peer.
                self.ledger.post(crate::ledger::LedgerEntry {
                    peer,
                    tier,
                    kind: crate::ledger::EntryKind::Debit,
                    amount: -2,
                    reason: "verification failed".to_string(),
                    related_job: Some(job.to_string()),
                    created_at_ms: now_ms,
                });
            }
            _ => {} // Pending/NotRequired — no reputation update.
        }

        guard.success();
        Ok(())
    }

    /// Aggregate pending verdicts for a job into a single outcome.
    ///
    /// ## Parameters
    /// - `job`: The job ID to aggregate verdicts for.
    ///
    /// ## Returns
    /// `Ok(VerificationResult)` — the aggregated outcome + all verdicts.
    /// Returns `VerificationOutcome::Pending` if no verdicts have been received
    /// or quorum hasn't been reached.
    ///
    /// ## Side Effects
    /// - If the aggregated outcome is final (Verified/Corrected), removes the
    ///   pending entry.
    async fn aggregate_verdicts(
        &self,
        job: &edgeforge_core::JobId,
    ) -> Result<VerificationResult, CrowdError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "coordinator.rs",
            "aggregate_verdicts",
        );

        let pending = self.pending.read();
        let entry = match pending.get(job) {
            Some(e) => e.clone(),
            None => {
                // No pending verdicts for this job → NotRequired (Tier 1).
                guard.success();
                return Ok(VerificationResult {
                    outcome: VerificationOutcome::NotRequired,
                    verdicts: Vec::new(),
                    quorum_reached: true,
                    deadline_elapsed: false,
                });
            }
        };
        drop(pending);

        let total_expected = entry.expected_verifiers.max(1);
        let quorum_reached = QuorumCalculator::quorum_reached(entry.verdicts.len(), total_expected);

        let result = VerdictAggregator::aggregate(
            &entry.verdicts,
            quorum_reached,
            entry.deadline_elapsed,
        );

        tracing::info!(
            job = %job,
            outcome = %result.outcome,
            verdict_count = result.verdicts.len(),
            quorum_reached = result.quorum_reached,
            "aggregate_verdicts: aggregated"
        );

        // If the outcome is final (Verified/Corrected), remove the pending entry.
        if result.outcome == VerificationOutcome::Verified
            || result.outcome == VerificationOutcome::Corrected
        {
            self.pending.write().remove(job);
        }

        guard.success();
        Ok(result)
    }

    /// Evict a peer from the mesh.
    async fn evict_peer(&self, peer: NodeId) -> Result<(), CrowdError> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "coordinator.rs",
            "evict_peer",
        );

        self.reputation.evict(peer);
        self.peers.write().remove(&peer);

        tracing::info!(peer = %peer, "evict_peer: peer evicted");

        Ok(())
    }

    /// Returns the execution mode.
    fn execution_mode(&self) -> &ExecutionMode {
        &self.execution_mode
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn peer() -> NodeId {
        NodeId(Ulid::new())
    }

    fn cap_with_expert(layer: u32, expert: u32) -> PeerCapability {
        PeerCapability {
            hosted_experts: vec![(LayerIdx(layer), ExpertId(expert))],
            ..Default::default()
        }
    }

    // ── ExecutionMode tests ──

    #[test]
    fn test_execution_mode_tier() {
        assert_eq!(ExecutionMode::Local.tier(), CrowdTier::Trusted);
        assert_eq!(ExecutionMode::DistributedTrusted.tier(), CrowdTier::Trusted);
        assert_eq!(
            ExecutionMode::DistributedCooperative {
                verification: VerificationPolicy::default(),
                redundancy: RedundancyPolicy::default(),
            }
            .tier(),
            CrowdTier::Cooperative
        );
    }

    #[test]
    fn test_execution_mode_verification_policy() {
        assert!(ExecutionMode::Local.verification_policy().is_none());
        assert!(ExecutionMode::DistributedTrusted.verification_policy().is_none());

        let mode = ExecutionMode::DistributedCooperative {
            verification: VerificationPolicy {
                deadline_ms: 50,
                sampler: crate::VerificationSampler::ProofOfQuality { sample_rate: 0.3 },
                redundancy: 2,
            },
            redundancy: RedundancyPolicy::default(),
        };
        let policy = mode.verification_policy().unwrap();
        assert_eq!(policy.deadline_ms, 50);
    }

    // ── CrowdError tests ──

    #[test]
    fn test_crowd_error_peer_not_found() {
        let p = peer();
        let err = CrowdError::PeerNotFound(p);
        assert!(err.to_string().contains("peer not found"));
    }

    #[test]
    fn test_crowd_error_no_peers_available() {
        let err = CrowdError::NoPeersAvailable(LayerIdx(0), ExpertId(1));
        assert!(err.to_string().contains("no peers available"));
    }

    #[test]
    fn test_crowd_error_to_edgeforge_error() {
        let err: EdgeForgeError = CrowdError::Internal("test".into()).into();
        assert!(matches!(err, EdgeForgeError::Mesh(_)));
    }

    // ── admit_peer tests ──

    #[tokio::test]
    async fn test_admit_peer_stores_capability() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p = peer();
        c.admit_peer(p, CrowdTier::Trusted, cap_with_expert(0, 1)).await.unwrap();
        assert_eq!(c.peer_count(), 1);
        assert_eq!(c.reputation.len(), 1);
    }

    #[tokio::test]
    async fn test_admit_peer_multiple() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        for _ in 0..5 {
            c.admit_peer(peer(), CrowdTier::Trusted, PeerCapability::default()).await.unwrap();
        }
        assert_eq!(c.peer_count(), 5);
    }

    // ── route_expert_fetch tests ──

    #[tokio::test]
    async fn test_route_expert_fetch_tier1_returns_hosting_peer() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p1 = peer();
        let p2 = peer();
        c.admit_peer(p1, CrowdTier::Trusted, cap_with_expert(0, 1)).await.unwrap();
        c.admit_peer(p2, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Trusted).await.unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], p1); // p1 hosts the expert
    }

    #[tokio::test]
    async fn test_route_expert_fetch_tier1_no_hosting_peer() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p1 = peer();
        c.admit_peer(p1, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Trusted).await.unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], p1); // best peer overall
    }

    #[tokio::test]
    async fn test_route_expert_fetch_no_peers_error() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let result = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Trusted).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrowdError::NoPeersAvailable(_, _)));
    }

    #[tokio::test]
    async fn test_route_expert_fetch_tier2_with_verifiers() {
        let mode = ExecutionMode::DistributedCooperative {
            verification: VerificationPolicy {
                deadline_ms: 50,
                sampler: crate::VerificationSampler::ProofOfQuality { sample_rate: 0.5 },
                redundancy: 2,
            },
            redundancy: RedundancyPolicy::default(),
        };
        let c = DefaultCrowdCoordinator::new(mode);

        // Admit 4 peers, one hosts the expert.
        let p1 = peer();
        let p2 = peer();
        let p3 = peer();
        let p4 = peer();
        c.admit_peer(p1, CrowdTier::Cooperative, cap_with_expert(0, 1)).await.unwrap();
        c.admit_peer(p2, CrowdTier::Cooperative, PeerCapability::default()).await.unwrap();
        c.admit_peer(p3, CrowdTier::Cooperative, PeerCapability::default()).await.unwrap();
        c.admit_peer(p4, CrowdTier::Cooperative, PeerCapability::default()).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Cooperative).await.unwrap();
        // p1 is the fetch peer (hosts the expert), plus PoQ-sampled verifiers.
        assert!(selected.len() >= 1);
        assert_eq!(selected[0], p1); // fetch peer first
        // With 3 verifier candidates and 0.5 sample rate → ceil(0.5 * 3) = 2 verifiers.
        assert!(selected.len() >= 2); // fetch + at least 1 verifier
    }

    // ── record_verdict tests ──

    #[tokio::test]
    async fn test_record_verdict_stores_verdict() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedCooperative {
            verification: VerificationPolicy::default(),
            redundancy: RedundancyPolicy::default(),
        });
        let p = peer();
        let job = edgeforge_core::JobId(Ulid::new());
        c.admit_peer(p, CrowdTier::Cooperative, PeerCapability::default()).await.unwrap();

        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job,
                peer: p,
                tier: CrowdTier::Cooperative,
                outcome: VerificationOutcome::Verified,
                latency_ms: 10,
            },
        };
        c.record_verdict(event).await.unwrap();

        // The verdict should be stored.
        assert_eq!(c.pending_job_count(), 1);
    }

    #[tokio::test]
    async fn test_record_verdict_updates_reputation_on_verified() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p = peer();
        let job = edgeforge_core::JobId(Ulid::new());
        c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        let initial_score = c.reputation.get(p).unwrap().quality_score;

        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job,
                peer: p,
                tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Verified,
                latency_ms: 10,
            },
        };
        c.record_verdict(event).await.unwrap();

        let after_score = c.reputation.get(p).unwrap().quality_score;
        assert!(after_score > initial_score, "Verified verdict should increase quality score");
        assert_eq!(c.reputation.get(p).unwrap().success_count, 1);
    }

    #[tokio::test]
    async fn test_record_verdict_updates_reputation_on_corrected() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p = peer();
        let job = edgeforge_core::JobId(Ulid::new());
        c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        let initial_score = c.reputation.get(p).unwrap().quality_score;

        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job,
                peer: p,
                tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Corrected,
                latency_ms: 10,
            },
        };
        c.record_verdict(event).await.unwrap();

        let after_score = c.reputation.get(p).unwrap().quality_score;
        assert!(after_score < initial_score, "Corrected verdict should decrease quality score");
        assert_eq!(c.reputation.get(p).unwrap().failure_count, 1);
    }

    #[tokio::test]
    async fn test_record_verdict_credits_ledger_on_verified() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p = peer();
        let job = edgeforge_core::JobId(Ulid::new());
        c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job,
                peer: p,
                tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Verified,
                latency_ms: 10,
            },
        };
        c.record_verdict(event).await.unwrap();

        assert_eq!(c.ledger.balance(p), 1); // credited 1
    }

    #[tokio::test]
    async fn test_record_verdict_debits_ledger_on_corrected() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p = peer();
        let job = edgeforge_core::JobId(Ulid::new());
        c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job,
                peer: p,
                tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Corrected,
                latency_ms: 10,
            },
        };
        c.record_verdict(event).await.unwrap();

        assert_eq!(c.ledger.balance(p), -2); // debited 2
    }

    #[tokio::test]
    async fn test_record_verdict_rejects_non_verification_event() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::TokenGenerated {
                sequence: 0,
                job: edgeforge_core::JobId(Ulid::new()),
                model: edgeforge_core::ModelId(Ulid::new()),
                tokens_so_far: 1,
            },
        };
        assert!(c.record_verdict(event).await.is_err());
    }

    // ── aggregate_verdicts tests ──

    #[tokio::test]
    async fn test_aggregate_verdicts_no_pending_returns_not_required() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let job = edgeforge_core::JobId(Ulid::new());
        let result = c.aggregate_verdicts(&job).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::NotRequired);
    }

    #[tokio::test]
    async fn test_aggregate_verdicts_all_verified() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let job = edgeforge_core::JobId(Ulid::new());

        // Record 3 Verified verdicts.
        for _ in 0..3 {
            let p = peer();
            c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();
            c.record_verdict(ControlEvent {
                timestamp_ms: 0,
                kind: ControlEventKind::VerificationVerdict {
                    job,
                    peer: p,
                    tier: CrowdTier::Trusted,
                    outcome: VerificationOutcome::Verified,
                    latency_ms: 5,
                },
            }).await.unwrap();
        }

        let result = c.aggregate_verdicts(&job).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::Verified);
        assert_eq!(result.verdicts.len(), 3);
    }

    #[tokio::test]
    async fn test_aggregate_verdicts_majority_corrected() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let job = edgeforge_core::JobId(Ulid::new());

        // 2 Corrected + 1 Verified → majority Corrected.
        let p1 = peer();
        let p2 = peer();
        let p3 = peer();
        c.admit_peer(p1, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();
        c.admit_peer(p2, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();
        c.admit_peer(p3, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();

        c.record_verdict(ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job, peer: p1, tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Corrected, latency_ms: 5,
            },
        }).await.unwrap();
        c.record_verdict(ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job, peer: p2, tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Corrected, latency_ms: 5,
            },
        }).await.unwrap();
        c.record_verdict(ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job, peer: p3, tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Verified, latency_ms: 5,
            },
        }).await.unwrap();

        let result = c.aggregate_verdicts(&job).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::Corrected);
    }

    #[tokio::test]
    async fn test_aggregate_verdicts_removes_finalized_job() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let job = edgeforge_core::JobId(Ulid::new());

        let p = peer();
        c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();
        c.record_verdict(ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job, peer: p, tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Verified, latency_ms: 5,
            },
        }).await.unwrap();

        assert_eq!(c.pending_job_count(), 1);
        let _ = c.aggregate_verdicts(&job).await.unwrap();
        // Finalized (Verified) → removed from pending.
        assert_eq!(c.pending_job_count(), 0);
    }

    // ── evict_peer tests ──

    #[tokio::test]
    async fn test_evict_peer_removes_from_reputation_and_peers() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::DistributedTrusted);
        let p = peer();
        c.admit_peer(p, CrowdTier::Trusted, PeerCapability::default()).await.unwrap();
        assert_eq!(c.peer_count(), 1);
        assert_eq!(c.reputation.len(), 1);

        c.evict_peer(p).await.unwrap();
        assert_eq!(c.peer_count(), 0);
        assert_eq!(c.reputation.len(), 0);
    }

    // ── execution_mode accessor ──

    #[tokio::test]
    async fn test_execution_mode_accessor() {
        let c = DefaultCrowdCoordinator::new(ExecutionMode::Local);
        assert!(matches!(c.execution_mode(), ExecutionMode::Local));
    }
}
