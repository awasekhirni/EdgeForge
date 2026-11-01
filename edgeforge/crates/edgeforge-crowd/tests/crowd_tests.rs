//! # Crowd Coordination Tests — edgeforge-crowd
//!
//! Tests for: reputation graph, incentive ledger, crowd coordinator,
//! PoQ sampling, quorum calculation, verdict aggregation, and the full
//! Tier 2/3 verification pipeline.

#[cfg(test)]
mod tests {
    use edgeforge_crowd::*;
    use edgeforge_core::{
        ControlEvent, ControlEventKind, CrowdTier as CoreCrowdTier, ExpertId, JobId, LayerIdx,
        NodeId, VerificationOutcome as CoreOutcome,
    };
    use ulid::Ulid;

    fn peer() -> NodeId {
        NodeId(Ulid::new())
    }

    fn job() -> JobId {
        JobId(Ulid::new())
    }

    fn cap_with_expert(layer: u32, expert: u32) -> verification::PeerCapability {
        verification::PeerCapability {
            hosted_experts: vec![(LayerIdx(layer), ExpertId(expert))],
            ..Default::default()
        }
    }

    fn verdict_event(
        job: JobId,
        peer: NodeId,
        outcome: CoreOutcome,
    ) -> ControlEvent {
        ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::VerificationVerdict {
                job,
                peer,
                tier: CoreCrowdTier::Trusted,
                outcome,
                latency_ms: 10,
            },
        }
    }

    // ── Reputation Tests ────────────────────────────────────────────

    #[test]
    fn test_reputation_admit_and_get() {
        let g = reputation::ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        let rep = g.get(p).unwrap();
        assert_eq!(rep.quality_score, 0.5);
        assert_eq!(rep.success_count, 0);
    }

    #[test]
    fn test_reputation_record_success_updates_score() {
        let g = reputation::ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        g.record_success(p, 10.0, 1);
        let rep = g.get(p).unwrap();
        assert_eq!(rep.success_count, 1);
        assert!(rep.quality_score > 0.5);
    }

    #[test]
    fn test_reputation_record_failure_decreases_score() {
        let g = reputation::ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Open, 0);
        g.record_failure(p, 1);
        g.record_failure(p, 2);
        let rep = g.get(p).unwrap();
        assert_eq!(rep.failure_count, 2);
        assert!(rep.quality_score < 0.5);
    }

    #[test]
    fn test_reputation_evict() {
        let g = reputation::ReputationGraph::new();
        let p = peer();
        g.admit(p, CrowdTier::Trusted, 0);
        assert_eq!(g.len(), 1);
        g.evict(p);
        assert!(g.get(p).is_none());
        assert_eq!(g.len(), 0);
    }

    // ── Ledger Tests ────────────────────────────────────────────────

    #[test]
    fn test_ledger_post_and_balance() {
        let l = ledger::IncentiveLedger::new();
        let p = peer();
        l.post(ledger::LedgerEntry {
            peer: p,
            tier: CrowdTier::Cooperative,
            kind: ledger::EntryKind::Credit,
            amount: 100,
            reason: "test".to_string(),
            related_job: None,
            created_at_ms: 0,
        });
        assert_eq!(l.balance(p), 100);
    }

    #[test]
    fn test_ledger_debit() {
        let l = ledger::IncentiveLedger::new();
        let p = peer();
        l.post(ledger::LedgerEntry {
            peer: p,
            tier: CrowdTier::Open,
            kind: ledger::EntryKind::Credit,
            amount: 50,
            reason: "init".to_string(),
            related_job: None,
            created_at_ms: 0,
        });
        l.post(ledger::LedgerEntry {
            peer: p,
            tier: CrowdTier::Open,
            kind: ledger::EntryKind::Debit,
            amount: -30,
            reason: "penalty".to_string(),
            related_job: None,
            created_at_ms: 1,
        });
        assert_eq!(l.balance(p), 20);
    }

    // ── Coordinator Tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_coordinator_admit_peer() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p = peer();
        c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
        assert_eq!(c.reputation.len(), 1);
        assert_eq!(c.peer_count(), 1);
    }

    #[tokio::test]
    async fn test_coordinator_evict_peer() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p = peer();
        c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
        c.evict_peer(p).await.unwrap();
        assert_eq!(c.reputation.len(), 0);
        assert_eq!(c.peer_count(), 0);
    }

    #[tokio::test]
    async fn test_coordinator_execution_mode() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::Local);
        assert!(matches!(c.execution_mode(), coordinator::ExecutionMode::Local));
    }

    // ── route_expert_fetch Tests ────────────────────────────────────

    #[tokio::test]
    async fn test_route_expert_fetch_tier1_returns_hosting_peer() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p1 = peer();
        let p2 = peer();
        c.admit_peer(p1, CrowdTier::Trusted, cap_with_expert(0, 1)).await.unwrap();
        c.admit_peer(p2, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Trusted).await.unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], p1);
    }

    #[tokio::test]
    async fn test_route_expert_fetch_no_peers_error() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let result = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Trusted).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_route_expert_fetch_tier2_with_verifiers() {
        let mode = coordinator::ExecutionMode::DistributedCooperative {
            verification: verification::VerificationPolicy {
                deadline_ms: 50,
                sampler: verification::VerificationSampler::ProofOfQuality { sample_rate: 0.5 },
                redundancy: 2,
            },
            redundancy: verification::RedundancyPolicy::default(),
        };
        let c = coordinator::DefaultCrowdCoordinator::new(mode);

        let p1 = peer();
        let p2 = peer();
        let p3 = peer();
        let p4 = peer();
        c.admit_peer(p1, CrowdTier::Cooperative, cap_with_expert(0, 1)).await.unwrap();
        c.admit_peer(p2, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();
        c.admit_peer(p3, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();
        c.admit_peer(p4, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Cooperative).await.unwrap();
        assert!(selected.len() >= 2); // fetch peer + at least 1 verifier
        assert_eq!(selected[0], p1); // fetch peer first
    }

    // ── record_verdict + aggregate_verdicts Tests ───────────────────

    #[tokio::test]
    async fn test_record_verdict_stores_verdict() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p = peer();
        let j = job();
        c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
        c.record_verdict(verdict_event(j, p, CoreOutcome::Verified)).await.unwrap();
        assert_eq!(c.pending_job_count(), 1);
    }

    #[tokio::test]
    async fn test_record_verdict_credits_ledger() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p = peer();
        let j = job();
        c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
        c.record_verdict(verdict_event(j, p, CoreOutcome::Verified)).await.unwrap();
        assert_eq!(c.ledger.balance(p), 1);
    }

    #[tokio::test]
    async fn test_record_verdict_debits_ledger() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p = peer();
        let j = job();
        c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
        c.record_verdict(verdict_event(j, p, CoreOutcome::Corrected)).await.unwrap();
        assert_eq!(c.ledger.balance(p), -2);
    }

    #[tokio::test]
    async fn test_aggregate_verdicts_no_pending_not_required() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let j = job();
        let result = c.aggregate_verdicts(&j).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::NotRequired);
    }

    #[tokio::test]
    async fn test_aggregate_verdicts_all_verified() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let j = job();
        for _ in 0..3 {
            let p = peer();
            c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
            c.record_verdict(verdict_event(j, p, CoreOutcome::Verified)).await.unwrap();
        }
        let result = c.aggregate_verdicts(&j).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::Verified);
        assert_eq!(result.verdicts.len(), 3);
    }

    #[tokio::test]
    async fn test_aggregate_verdicts_majority_corrected() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let j = job();
        for i in 0..3 {
            let p = peer();
            c.admit_peer(p, CrowdTier::Trusted, verification::PeerCapability::default()).await.unwrap();
            let outcome = if i < 2 { CoreOutcome::Corrected } else { CoreOutcome::Verified };
            c.record_verdict(verdict_event(j, p, outcome)).await.unwrap();
        }
        let result = c.aggregate_verdicts(&j).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::Corrected);
    }

    // ── End-to-End Tier 2 Verification Pipeline ─────────────────────

    #[tokio::test]
    async fn e2e_tier2_verification_pipeline() {
        // 1. Set up a Tier 2 coordinator with PoQ sampling.
        let mode = coordinator::ExecutionMode::DistributedCooperative {
            verification: verification::VerificationPolicy {
                deadline_ms: 50,
                sampler: verification::VerificationSampler::ProofOfQuality { sample_rate: 0.5 },
                redundancy: 2,
            },
            redundancy: verification::RedundancyPolicy::default(),
        };
        let c = coordinator::DefaultCrowdCoordinator::new(mode);

        // 2. Admit peers — one hosts the expert, others are verifiers.
        let fetch_peer = peer();
        let v1 = peer();
        let v2 = peer();
        let v3 = peer();
        c.admit_peer(fetch_peer, CrowdTier::Cooperative, cap_with_expert(0, 1)).await.unwrap();
        c.admit_peer(v1, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();
        c.admit_peer(v2, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();
        c.admit_peer(v3, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();

        // 3. Route the expert fetch — should return fetch peer + verifiers.
        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Cooperative).await.unwrap();
        assert!(selected.len() >= 2);
        assert_eq!(selected[0], fetch_peer);

        // 4. Simulate verifiers submitting verdicts (all Verified).
        let j = job();
        for p in selected.iter().skip(1) {
            c.record_verdict(verdict_event(j, *p, CoreOutcome::Verified)).await.unwrap();
        }

        // 5. Aggregate the verdicts — should be Verified.
        let result = c.aggregate_verdicts(&j).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::Verified);
        assert!(result.quorum_reached);

        // 6. The fetch peer's reputation should not have changed (it wasn't verified).
        // The verifiers' reputations should have improved (they passed verification).
        let v1_rep = c.reputation.get(v1).unwrap();
        assert!(v1_rep.success_count > 0);

        // 7. The finalized job should be removed from pending.
        assert_eq!(c.pending_job_count(), 0);
    }

    #[tokio::test]
    async fn e2e_tier2_verification_failure_corrected() {
        let mode = coordinator::ExecutionMode::DistributedCooperative {
            verification: verification::VerificationPolicy {
                deadline_ms: 50,
                sampler: verification::VerificationSampler::ProofOfQuality { sample_rate: 1.0 },
                redundancy: 1,
            },
            redundancy: verification::RedundancyPolicy::default(),
        };
        let c = coordinator::DefaultCrowdCoordinator::new(mode);

        let fetch_peer = peer();
        let v1 = peer();
        let v2 = peer();
        let v3 = peer();
        c.admit_peer(fetch_peer, CrowdTier::Cooperative, cap_with_expert(0, 1)).await.unwrap();
        c.admit_peer(v1, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();
        c.admit_peer(v2, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();
        c.admit_peer(v3, CrowdTier::Cooperative, verification::PeerCapability::default()).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Cooperative).await.unwrap();
        let j = job();

        // 2 verifiers say Corrected, 1 says Verified → majority Corrected.
        let verifiers: Vec<NodeId> = selected.iter().skip(1).copied().collect();
        for (i, p) in verifiers.iter().enumerate() {
            let outcome = if i < 2 { CoreOutcome::Corrected } else { CoreOutcome::Verified };
            c.record_verdict(verdict_event(j, *p, outcome)).await.unwrap();
        }

        let result = c.aggregate_verdicts(&j).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::Corrected);
    }

    #[tokio::test]
    async fn e2e_tier1_no_verification_not_required() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::DistributedTrusted);
        let p = peer();
        c.admit_peer(p, CrowdTier::Trusted, cap_with_expert(0, 1)).await.unwrap();

        let selected = c.route_expert_fetch(LayerIdx(0), ExpertId(1), CrowdTier::Trusted).await.unwrap();
        assert_eq!(selected.len(), 1);

        // No verdicts recorded → aggregate returns NotRequired.
        let j = job();
        let result = c.aggregate_verdicts(&j).await.unwrap();
        assert_eq!(result.outcome, VerificationOutcome::NotRequired);
    }

    // ── PoQ Sampler Tests ───────────────────────────────────────────

    #[test]
    fn test_poq_sampler_empty() {
        assert!(verification::PoQSampler::sample(&[], 0.5).is_empty());
    }

    #[test]
    fn test_poq_sampler_selects_top_k() {
        let p1 = peer();
        let p2 = peer();
        let p3 = peer();
        let candidates = vec![(p1, 0.3), (p2, 0.9), (p3, 0.6)];
        let selected = verification::PoQSampler::sample(&candidates, 0.34);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0], p2); // highest quality first
    }

    // ── Quorum Calculator Tests ─────────────────────────────────────

    #[test]
    fn test_quorum_threshold() {
        assert_eq!(verification::QuorumCalculator::threshold(0), 0);
        assert_eq!(verification::QuorumCalculator::threshold(5), 4);
        assert_eq!(verification::QuorumCalculator::threshold(10), 6);
    }

    #[test]
    fn test_quorum_reached() {
        assert!(!verification::QuorumCalculator::quorum_reached(2, 5));
        assert!(verification::QuorumCalculator::quorum_reached(4, 5));
    }

    // ── Verdict Aggregator Tests ────────────────────────────────────

    #[test]
    fn test_aggregate_all_verified() {
        let verdicts = vec![
            verification::PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 5, timestamp_ms: 0 },
            verification::PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 8, timestamp_ms: 1 },
        ];
        let result = verification::VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.outcome, VerificationOutcome::Verified);
    }

    #[test]
    fn test_aggregate_majority_corrected() {
        let verdicts = vec![
            verification::PeerVerdict { peer: peer(), outcome: VerificationOutcome::Corrected, latency_ms: 5, timestamp_ms: 0 },
            verification::PeerVerdict { peer: peer(), outcome: VerificationOutcome::Corrected, latency_ms: 8, timestamp_ms: 1 },
            verification::PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 6, timestamp_ms: 2 },
        ];
        let result = verification::VerdictAggregator::aggregate(&verdicts, true, false);
        assert_eq!(result.outcome, VerificationOutcome::Corrected);
    }

    #[test]
    fn test_aggregate_deadline_no_quorum_pending() {
        let verdicts = vec![
            verification::PeerVerdict { peer: peer(), outcome: VerificationOutcome::Verified, latency_ms: 5, timestamp_ms: 0 },
        ];
        let result = verification::VerdictAggregator::aggregate(&verdicts, false, true);
        assert_eq!(result.outcome, VerificationOutcome::Pending);
    }
}
