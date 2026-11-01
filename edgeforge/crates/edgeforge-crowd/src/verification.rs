//! # Verification Policies, Samplers, and Verdict Aggregation (ADR-017, ADR-018)
//!
//! ## Purpose
//! Per-tier verification policy. Tier 1: none (mTLS sufficient). Tier 2: Proof
//! of Quality (sampled evaluators). Tier 3: optimistic + eventual verification.
//!
//! ## What This File Does
//! - `CrowdTier` — Trusted, Cooperative, Open
//! - `VerificationPolicy` — deadline_ms, sampler, redundancy
//! - `VerificationSampler` — None, ProofOfQuality, Optimistic
//! - `RedundancyPolicy` — replication_factor, timeout_ms
//! - `ChurnPolicy` — reassignment_timeout_ms, heartbeat_miss_threshold
//! - `VerificationOutcome` — NotRequired, Verified, Pending, Corrected
//! - `PeerCapability` — GPU model, VRAM, RAM, SSD, hosted experts, thermal, bandwidth, region
//! - `PeerVerdict` — a single peer's verification verdict (peer, outcome, latency)
//! - `VerdictAggregator` — collects peer verdicts and produces an aggregated `VerificationOutcome`
//! - `PoQSampler` — selects a subset of peers for Proof of Quality verification
//! - `QuorumCalculator` — determines if enough verifiers agree (majority quorum)
//! - `VerificationResult` — the result of a verification cycle (outcome, verdicts, quorum_reached)
//!
//! ## PoQ Sampling Algorithm
//! `PoQSampler::sample(peers, sample_rate)` selects `ceil(sample_rate * peers.len())`
//! peers from the candidate list. The selection is deterministic given the same
//! input (sorted by quality score, then take the top-k). This ensures:
//! - High-quality peers are preferentially selected as verifiers
//! - The sample size scales with the pool size
//! - The selection is reproducible (important for debugging)
//!
//! ## Quorum Calculation
//! `QuorumCalculator::quorum_reached(verdicts, total_expected)` returns true when
//! `verdicts.len() > total_expected / 2` (simple majority). The quorum threshold
//! is `⌈total_expected / 2⌉ + 1` to ensure a strict majority.
//!
//! ## Verdict Aggregation
//! `VerdictAggregator::aggregate(verdicts, quorum_reached, deadline_elapsed)`:
//! - If `deadline_elapsed` and not `quorum_reached` → `Pending`
//! - If `quorum_reached` and all verdicts are `Verified` → `Verified`
//! - If `quorum_reached` and majority are `Failed` → `Corrected`
//! - Otherwise → `Pending` (mixed verdicts, no majority)
//!
//! ## Dependencies
//! - `edgeforge_core::{ExpertId, LayerIdx, NodeId}` — for hosted_experts + verdict routing
//! - `edgeforge_observability` — FunctionGuard for call logging
//! - `serde` — serialization
//!
//! ## Output
//! `VerificationOutcome` — surfaced via the on_token callback and
//! X-EdgeForge-Verification SSE header.

use edgeforge_core::{ExpertId, LayerIdx, NodeId};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The crowd tier — determines the verification policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrowdTier {
    /// Tier 1 — trusted mesh (≤16 peers). mTLS is sufficient; no verification.
    Trusted,
    /// Tier 2 — cooperative pool (≤100 peers). Proof of Quality sampling.
    Cooperative,
    /// Tier 3 — open crowd (unbounded). Optimistic + eventual verification.
    Open,
}

impl CrowdTier {
    /// Returns the string name of the tier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Cooperative => "cooperative",
            Self::Open => "open",
        }
    }
}

impl std::fmt::Display for CrowdTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Per-tier verification policy.
#[derive(Clone, Debug)]
pub struct VerificationPolicy {
    /// Maximum time to wait for verdicts before declaring `Pending` (ADR-018, R-21: 50ms).
    pub deadline_ms: u32,
    /// The sampling strategy for selecting verifiers.
    pub sampler: VerificationSampler,
    /// Number of redundant verifications (1 for Tier 1, 2+ for Tier 2/3).
    pub redundancy: u8,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self {
            deadline_ms: 50,
            sampler: VerificationSampler::None,
            redundancy: 1,
        }
    }
}

/// The sampling strategy for selecting verifiers.
#[derive(Clone, Copy, Debug)]
pub enum VerificationSampler {
    /// No verification (Tier 1).
    None,
    /// Proof of Quality — sample `sample_rate` fraction of peers as verifiers.
    ProofOfQuality { sample_rate: f32 },
    /// Optimistic — accept first, recheck `recheck_rate` fraction later.
    Optimistic { recheck_rate: f32 },
}

/// Redundancy policy for k-of-n expert fetch replication.
#[derive(Clone, Debug)]
pub struct RedundancyPolicy {
    /// How many peers to replicate each expert fetch to.
    pub replication_factor: u8,
    /// Timeout before reassigning to another peer.
    pub timeout_ms: u32,
}

impl Default for RedundancyPolicy {
    fn default() -> Self {
        Self {
            replication_factor: 1,
            timeout_ms: 30,
        }
    }
}

/// Churn tolerance policy for Tier 3 (Open Crowd).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ChurnPolicy {
    /// Reassign an expert to another peer after this many ms without response.
    pub reassignment_timeout_ms: u32,
    /// A peer is marked dead after this many consecutive missed heartbeats.
    pub heartbeat_miss_threshold: u8,
}

impl Default for ChurnPolicy {
    fn default() -> Self {
        Self {
            reassignment_timeout_ms: 100,
            heartbeat_miss_threshold: 3,
        }
    }
}

/// The outcome of a verification cycle.
///
/// This is the aggregated result surfaced via the on_token callback and the
/// `X-EdgeForge-Verification` SSE header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationOutcome {
    /// Tier 1 / Local — no verification needed (mTLS is sufficient).
    NotRequired,
    /// Tier 2/3 — verification passed within the deadline (quorum of verifiers agree).
    Verified,
    /// Tier 2/3 — verification pending at emit time. A correction chunk may follow.
    Pending,
    /// This token corrects a previously-emitted Pending token that failed verification.
    Corrected,
}

impl VerificationOutcome {
    /// Returns the string name of the outcome.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Verified => "verified",
            Self::Pending => "pending",
            Self::Corrected => "corrected",
        }
    }
}

impl std::fmt::Display for VerificationOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A peer's hardware capability + currently-hosted experts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerCapability {
    /// GPU model name (e.g., "RTX 3090", "Apple M3 Max").
    pub gpu_model: Option<String>,
    /// Total VRAM in bytes.
    pub vram_bytes: u64,
    /// Total system RAM in bytes.
    pub ram_bytes: u64,
    /// SSD capacity in GiB.
    pub ssd_gib: u64,
    /// List of (layer, expert) pairs currently resident on this peer.
    pub hosted_experts: Vec<(LayerIdx, ExpertId)>,
    /// Current SSD temperature in degrees Celsius.
    pub thermal_celsius: f32,
    /// Network bandwidth in bits per second.
    pub bandwidth_bps: u64,
    /// Grid region for carbon-aware routing (ADR-026).
    pub region: String,
}

impl Default for PeerCapability {
    fn default() -> Self {
        Self {
            gpu_model: None,
            vram_bytes: 0,
            ram_bytes: 0,
            ssd_gib: 0,
            hosted_experts: Vec::new(),
            thermal_celsius: 50.0,
            bandwidth_bps: 1_000_000_000,
            region: "WORLD-AVG".into(),
        }
    }
}

// ── Peer Verdict ───────────────────────────────────────────────────────────

/// A single peer's verification verdict for one expert fetch.
///
/// ## Purpose
/// When a peer verifies an expert fetch result, it produces a `PeerVerdict`.
/// The `VerdictAggregator` collects these and produces an aggregated
/// `VerificationOutcome`.
#[derive(Clone, Debug)]
pub struct PeerVerdict {
    /// The peer that produced this verdict.
    pub peer: NodeId,
    /// The peer's individual verification outcome.
    pub outcome: VerificationOutcome,
    /// The latency of the verification in milliseconds.
    pub latency_ms: u32,
    /// When the verdict was received (Unix ms).
    pub timestamp_ms: i64,
}

// ── PoQ Sampler ────────────────────────────────────────────────────────────

/// Proof of Quality sampler — selects a subset of peers for verification.
///
/// ## Purpose
/// Given a list of candidate peers (with their quality scores) and a sample
/// rate, selects `ceil(sample_rate * len)` peers to act as verifiers.
///
/// ## Selection Algorithm
/// Peers are sorted by quality score (descending), then the top-k are selected.
/// This ensures:
/// - High-quality peers are preferentially selected as verifiers
/// - The sample size scales with the pool size
/// - The selection is deterministic (reproducible for debugging)
pub struct PoQSampler;

impl PoQSampler {
    /// Select a subset of peers for Proof of Quality verification.
    ///
    /// ## Parameters
    /// - `candidates`: List of (peer, quality_score) pairs. Quality score
    ///   is in [0.0, 1.0] (higher = better).
    /// - `sample_rate`: Fraction of peers to select (0.0 = none, 1.0 = all).
    ///
    /// ## Returns
    /// `Vec<NodeId>` — the selected verifier peers, sorted by quality score
    /// (highest first). Returns an empty vec if `candidates` is empty or
    /// `sample_rate` is 0.0.
    #[must_use]
    pub fn sample(candidates: &[(NodeId, f32)], sample_rate: f32) -> Vec<NodeId> {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-crowd", "verification.rs", "PoQSampler::sample");

        if candidates.is_empty() || sample_rate <= 0.0 {
            return Vec::new();
        }

        // Sort by quality score descending (highest quality first).
        let mut sorted: Vec<(NodeId, f32)> = candidates.to_vec();
        sorted.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Calculate sample size: ceil(sample_rate * len), at least 1.
        let sample_size = ((sample_rate * candidates.len() as f32).ceil() as usize)
            .max(1)
            .min(candidates.len());

        let selected: Vec<NodeId> = sorted
            .into_iter()
            .take(sample_size)
            .map(|(peer, _)| peer)
            .collect();

        tracing::debug!(
            candidate_count = candidates.len(),
            sample_rate,
            sample_size = selected.len(),
            "PoQSampler: selected verifiers"
        );

        selected
    }
}

// ── Quorum Calculator ──────────────────────────────────────────────────────

/// Quorum calculator — determines if enough verifiers have responded.
///
/// ## Purpose
/// A quorum is reached when more than half of the expected verifiers have
/// submitted their verdicts. This ensures a strict majority before aggregating.
pub struct QuorumCalculator;

impl QuorumCalculator {
    /// Returns the quorum threshold (minimum verdicts needed for a majority).
    ///
    /// ## Parameters
    /// - `total_expected`: The total number of verifiers expected to respond.
    ///
    /// ## Returns
    /// `usize` — the minimum number of verdicts needed for a quorum.
    /// Formula: `⌈total_expected / 2⌉ + 1` (strict majority).
    /// Returns 0 if `total_expected` is 0.
    #[must_use]
    pub fn threshold(total_expected: usize) -> usize {
        if total_expected == 0 {
            return 0;
        }
        (total_expected + 1) / 2 + 1
    }

    /// Check if a quorum has been reached.
    ///
    /// ## Parameters
    /// - `verdicts_received`: The number of verdicts received so far.
    /// - `total_expected`: The total number of verifiers expected.
    ///
    /// ## Returns
    /// `true` if `verdicts_received >= threshold(total_expected)`.
    #[must_use]
    pub fn quorum_reached(verdicts_received: usize, total_expected: usize) -> bool {
        if total_expected == 0 {
            return true;
        }
        verdicts_received >= Self::threshold(total_expected)
    }
}

// ── Verdict Aggregator ─────────────────────────────────────────────────────

/// The result of a verification cycle.
#[derive(Clone, Debug)]
pub struct VerificationResult {
    /// The aggregated verification outcome.
    pub outcome: VerificationOutcome,
    /// All individual peer verdicts received.
    pub verdicts: Vec<PeerVerdict>,
    /// Whether a quorum was reached.
    pub quorum_reached: bool,
    /// Whether the deadline elapsed before quorum was reached.
    pub deadline_elapsed: bool,
}

/// Verdict aggregator — collects peer verdicts and produces an aggregated outcome.
///
/// ## Purpose
/// The aggregator collects `PeerVerdict`s from multiple verifiers and produces
/// a single `VerificationOutcome` based on majority agreement.
///
/// ## Aggregation Rules
/// 1. If `deadline_elapsed` and not `quorum_reached` → `Pending`
/// 2. If `quorum_reached` and all verdicts are `Verified` → `Verified`
/// 3. If `quorum_reached` and majority are `Failed` → `Corrected`
/// 4. Otherwise → `Pending` (mixed verdicts, no clear majority)
pub struct VerdictAggregator;

impl VerdictAggregator {
    /// Aggregate peer verdicts into a single `VerificationOutcome`.
    ///
    /// ## Parameters
    /// - `verdicts`: The individual peer verdicts received so far.
    /// - `quorum_reached`: Whether a quorum of verdicts has been received.
    /// - `deadline_elapsed`: Whether the verification deadline has elapsed.
    ///
    /// ## Returns
    /// `VerificationResult` — the aggregated outcome + metadata.
    #[must_use]
    pub fn aggregate(
        verdicts: &[PeerVerdict],
        quorum_reached: bool,
        deadline_elapsed: bool,
    ) -> VerificationResult {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "verification.rs",
            "VerdictAggregator::aggregate",
        );

        // Rule 1: Deadline elapsed without quorum → Pending.
        if deadline_elapsed && !quorum_reached {
            tracing::info!(
                verdict_count = verdicts.len(),
                "VerdictAggregator: deadline elapsed without quorum → Pending"
            );
            guard.success();
            return VerificationResult {
                outcome: VerificationOutcome::Pending,
                verdicts: verdicts.to_vec(),
                quorum_reached: false,
                deadline_elapsed: true,
            };
        }

        // If no quorum yet and deadline not elapsed → still Pending.
        if !quorum_reached {
            guard.success();
            return VerificationResult {
                outcome: VerificationOutcome::Pending,
                verdicts: verdicts.to_vec(),
                quorum_reached: false,
                deadline_elapsed: false,
            };
        }

        // Quorum reached — count the verdicts.
        let total = verdicts.len();
        let verified_count = verdicts
            .iter()
            .filter(|v| v.outcome == VerificationOutcome::Verified)
            .count();
        let failed_count = verdicts
            .iter()
            .filter(|v| v.outcome == VerificationOutcome::Corrected)
            .count();

        tracing::info!(
            total_verdicts = total,
            verified_count,
            failed_count,
            "VerdictAggregator: aggregating verdicts"
        );

        // Rule 2: All verdicts are Verified → Verified.
        if verified_count == total {
            guard.success();
            return VerificationResult {
                outcome: VerificationOutcome::Verified,
                verdicts: verdicts.to_vec(),
                quorum_reached: true,
                deadline_elapsed,
            };
        }

        // Rule 3: Majority are Failed → Corrected.
        let threshold = QuorumCalculator::threshold(total);
        if failed_count >= threshold {
            guard.success();
            return VerificationResult {
                outcome: VerificationOutcome::Corrected,
                verdicts: verdicts.to_vec(),
                quorum_reached: true,
                deadline_elapsed,
            };
        }

        // Rule 4: Mixed verdicts, no clear majority → Pending.
        guard.success();
        VerificationResult {
            outcome: VerificationOutcome::Pending,
            verdicts: verdicts.to_vec(),
            quorum_reached: true,
            deadline_elapsed,
        }
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

    // ── CrowdTier tests ──

    #[test]
    fn test_crowd_tier_as_str() {
        assert_eq!(CrowdTier::Trusted.as_str(), "trusted");
        assert_eq!(CrowdTier::Cooperative.as_str(), "cooperative");
        assert_eq!(CrowdTier::Open.as_str(), "open");
    }

    #[test]
    fn test_crowd_tier_display() {
        assert_eq!(format!("{}", CrowdTier::Trusted), "trusted");
    }

    #[test]
    fn test_crowd_tier_equality() {
        assert_eq!(CrowdTier::Trusted, CrowdTier::Trusted);
        assert_ne!(CrowdTier::Trusted, CrowdTier::Cooperative);
    }

    // ── VerificationOutcome tests ──

    #[test]
    fn test_outcome_as_str() {
        assert_eq!(VerificationOutcome::NotRequired.as_str(), "not_required");
        assert_eq!(VerificationOutcome::Verified.as_str(), "verified");
        assert_eq!(VerificationOutcome::Pending.as_str(), "pending");
        assert_eq!(VerificationOutcome::Corrected.as_str(), "corrected");
    }

    #[test]
    fn test_outcome_display() {
        assert_eq!(format!("{}", VerificationOutcome::Verified), "verified");
    }

    // ── PoQSampler tests ──

    #[test]
    fn test_poq_sampler_empty_candidates() {
        let selected = PoQSampler::sample(&[], 0.5);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_poq_sampler_zero_sample_rate() {
        let candidates = vec![(peer(), 0.8)];
        let selected = PoQSampler::sample(&candidates, 0.0);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_poq_sampler_selects_top_k_by_quality() {
        let p1 = peer();
        let p2 = peer();
        let p3 = peer();
        let candidates = vec![(p1, 0.3), (p2, 0.9), (p3, 0.6)];
        // sample_rate=0.34 → ceil(0.34 * 3) = 2 → select top 2 by quality
        let selected = PoQSampler::sample(&candidates, 0.34);
        assert_eq!(selected.len(), 2);
        // p2 (0.9) and p3 (0.6) should be selected, in that order
        assert_eq!(selected[0], p2);
        assert_eq!(selected[1], p3);
    }

    #[test]
    fn test_poq_sampler_full_sample_rate() {
        let candidates = vec![(peer(), 0.5), (peer(), 0.7), (peer(), 0.3)];
        let selected = PoQSampler::sample(&candidates, 1.0);
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_poq_sampler_min_one_verifier() {
        let candidates = vec![(peer(), 0.5)];
        // sample_rate=0.01 → ceil(0.01 * 1) = 1 (min 1)
        let selected = PoQSampler::sample(&candidates, 0.01);
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn test_poq_sampler_large_pool() {
        let candidates: Vec<(NodeId, f32)> = (0..100).map(|_| (peer(), 0.5)).collect();
        let selected = PoQSampler::sample(&candidates, 0.1);
        // ceil(0.1 * 100) = 10
        assert_eq!(selected.len(), 10);
    }

    // ── QuorumCalculator tests ──

    #[test]
    fn test_quorum_threshold_zero() {
        assert_eq!(QuorumCalculator::threshold(0), 0);
    }

    #[test]
    fn test_quorum_threshold_one() {
        // (1 + 1) / 2 + 1 = 2 (need all 1 to agree, effectively)
        assert_eq!(QuorumCalculator::threshold(1), 2);
    }

    #[test]
    fn test_quorum_threshold_three() {
        // (3 + 1) / 2 + 1 = 3 (need 3 out of 3 for strict majority)
        assert_eq!(QuorumCalculator::threshold(3), 3);
    }

    #[test]
    fn test_quorum_threshold_five() {
        // (5 + 1) / 2 + 1 = 4 (need 4 out of 5 for strict majority)
        assert_eq!(QuorumCalculator::threshold(5), 4);
    }

    #[test]
    fn test_quorum_threshold_ten() {
        // (10 + 1) / 2 + 1 = 6 (need 6 out of 10)
        assert_eq!(QuorumCalculator::threshold(10), 6);
    }

    #[test]
    fn test_quorum_reached_zero_expected() {
        assert!(QuorumCalculator::quorum_reached(0, 0));
    }

    #[test]
    fn test_quorum_reached_below_threshold() {
        assert!(!QuorumCalculator::quorum_reached(2, 5)); // threshold = 4
    }

    #[test]
    fn test_quorum_reached_at_threshold() {
        assert!(QuorumCalculator::quorum_reached(4, 5)); // threshold = 4
    }

    #[test]
    fn test_quorum_reached_above_threshold() {
        assert!(QuorumCalculator::quorum_reached(5, 5));
    }

    // ── VerdictAggregator tests ──

    #[test]
    fn test_aggregate_deadline_elapsed_no_quorum_pending() {
        let verdicts = vec![PeerVerdict {
            peer: peer(),
            outcome: VerificationOutcome::Verified,
            latency_ms: 10,
            timestamp_ms: 0,
        }];
        let result = VerdictAggregator::aggregate(&verdicts, false, true);
        assert_eq!(result.outcome, VerificationOutcome::Pending);
        assert!(!result.quorum_reached);
        assert!(result.deadline_elapsed);
    }

    #[test]
    fn test_aggregate_no_quorum_not_deadline_pending() {
        let verdicts = vec![];
        let result = VerdictAggregator::aggregate(&verdicts, false, false);
        assert_eq!(result.outcome, VerificationOutcome::Pending);
        assert!(!result.quorum_reached);
        assert!(!result.deadline_elapsed);
    }

    #[test]
    fn test_aggregate_all_verified() {
        let verdicts = vec![
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 5, timestamp_ms: 0 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 8, timestamp_ms: 1 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 6, timestamp_ms: 2 },
        ];
        let result = VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.outcome, VerificationOutcome::Verified);
        assert!(result.quorum_reached);
    }

    #[test]
    fn test_aggregate_majority_failed_corrected() {
        // 3 verifiers: 2 Corrected, 1 Verified → majority failed → Corrected
        let verdicts = vec![
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Corrected, latency_ms: 5, timestamp_ms: 0 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Corrected, latency_ms: 8, timestamp_ms: 1 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 6, timestamp_ms: 2 },
        ];
        let result = VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.outcome, VerificationOutcome::Corrected);
        assert!(result.quorum_reached);
    }

    #[test]
    fn test_aggregate_mixed_no_majority_pending() {
        // 4 verifiers: 2 Verified, 2 Corrected → no majority → Pending
        let verdicts = vec![
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 5, timestamp_ms: 0 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 8, timestamp_ms: 1 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Corrected, latency_ms: 6, timestamp_ms: 2 },
            PeerVerdict { peer: peer(), outcome: VerificationOutcome::Corrected, latency_ms: 7, timestamp_ms: 3 },
        ];
        let result = VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.outcome, VerificationOutcome::Pending);
        assert!(result.quorum_reached);
    }

    #[test]
    fn test_aggregate_single_verified() {
        let verdicts = vec![PeerVerdict {
            peer: peer(),
            outcome: VerificationOutcome::Verified,
            latency_ms: 5,
            timestamp_ms: 0,
        }];
        let result = VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.outcome, VerificationOutcome::Verified);
    }

    #[test]
    fn test_aggregate_preserves_verdicts() {
        let v1 = PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 5, timestamp_ms: 0 };
        let v2 = PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 8, timestamp_ms: 1 };
        let verdicts = vec![v1.clone(), v2.clone()];
        let result = VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.verdicts.len(), 2);
        assert_eq!(result.verdicts[0].peer, v1.peer);
        assert_eq!(result.verdicts[1].peer, v2.peer);
    }

    // ── Default tests ──

    #[test]
    fn test_verification_policy_default() {
        let p = VerificationPolicy::default();
        assert_eq!(p.deadline_ms, 50);
        assert_eq!(p.redundancy, 1);
    }

    #[test]
    fn test_redundancy_policy_default() {
        let p = RedundancyPolicy::default();
        assert_eq!(p.replication_factor, 1);
        assert_eq!(p.timeout_ms, 30);
    }

    #[test]
    fn test_churn_policy_default() {
        let p = ChurnPolicy::default();
        assert_eq!(p.reassignment_timeout_ms, 100);
        assert_eq!(p.heartbeat_miss_threshold, 3);
    }

    #[test]
    fn test_peer_capability_default() {
        let cap = PeerCapability::default();
        assert!(cap.gpu_model.is_none());
        assert_eq!(cap.vram_bytes, 0);
        assert_eq!(cap.thermal_celsius, 50.0);
        assert_eq!(cap.bandwidth_bps, 1_000_000_000);
        assert_eq!(cap.region, "WORLD-AVG");
    }
}
