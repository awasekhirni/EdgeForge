// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Crowd Coordination Tests — edgeforge-crowd
//!
//! Tests for: reputation graph, incentive ledger, crowd coordinator.

#[cfg(test)]
mod tests {
    use edgeforge_core::NodeId;
    use edgeforge_crowd::*;
    use ulid::Ulid;

    // ── Reputation Tests ────────────────────────────────────────────

    #[test]
    fn test_reputation_admit_and_get() {
        let g = reputation::ReputationGraph::new();
        let peer = NodeId(Ulid::new());
        g.admit(peer, CrowdTier::Trusted, 0);
        let rep = g.get(peer).unwrap();
        assert_eq!(rep.quality_score, 0.5);
        assert_eq!(rep.success_count, 0);
    }

    #[test]
    fn test_reputation_record_success_updates_score() {
        let g = reputation::ReputationGraph::new();
        let peer = NodeId(Ulid::new());
        g.admit(peer, CrowdTier::Trusted, 0);
        g.record_success(peer, 10.0, 1);
        let rep = g.get(peer).unwrap();
        assert_eq!(rep.success_count, 1);
        assert!(rep.quality_score > 0.5);
    }

    #[test]
    fn test_reputation_record_failure_decreases_score() {
        let g = reputation::ReputationGraph::new();
        let peer = NodeId(Ulid::new());
        g.admit(peer, CrowdTier::Open, 0);
        g.record_failure(peer, 1);
        g.record_failure(peer, 2);
        let rep = g.get(peer).unwrap();
        assert_eq!(rep.failure_count, 2);
        assert!(rep.quality_score < 0.5);
    }

    #[test]
    fn test_reputation_evict() {
        let g = reputation::ReputationGraph::new();
        let peer = NodeId(Ulid::new());
        g.admit(peer, CrowdTier::Trusted, 0);
        assert_eq!(g.len(), 1);
        g.evict(peer);
        assert!(g.get(peer).is_none());
        assert_eq!(g.len(), 0);
    }

    // ── Ledger Tests ────────────────────────────────────────────────

    #[test]
    fn test_ledger_post_and_balance() {
        let l = ledger::IncentiveLedger::new();
        let peer = NodeId(Ulid::new());
        l.post(ledger::LedgerEntry {
            peer,
            tier: CrowdTier::Cooperative,
            kind: ledger::EntryKind::Credit,
            amount: 100,
            reason: "test".to_string(),
            related_job: None,
            created_at_ms: 0,
        });
        assert_eq!(l.balance(peer), 100);
    }

    #[test]
    fn test_ledger_debit() {
        let l = ledger::IncentiveLedger::new();
        let peer = NodeId(Ulid::new());
        l.post(ledger::LedgerEntry {
            peer,
            tier: CrowdTier::Open,
            kind: ledger::EntryKind::Credit,
            amount: 50,
            reason: "init".to_string(),
            related_job: None,
            created_at_ms: 0,
        });
        l.post(ledger::LedgerEntry {
            peer,
            tier: CrowdTier::Open,
            kind: ledger::EntryKind::Debit,
            amount: -30,
            reason: "penalty".to_string(),
            related_job: None,
            created_at_ms: 1,
        });
        assert_eq!(l.balance(peer), 20);
    }

    // ── Coordinator Tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_coordinator_admit_peer() {
        let c = coordinator::DefaultCrowdCoordinator::new(
            coordinator::ExecutionMode::DistributedTrusted,
        );
        let peer = NodeId(Ulid::new());
        c.admit_peer(
            peer,
            CrowdTier::Trusted,
            verification::PeerCapability::default(),
        )
        .await
        .unwrap();
        assert_eq!(c.reputation.len(), 1);
    }

    #[tokio::test]
    async fn test_coordinator_evict_peer() {
        let c = coordinator::DefaultCrowdCoordinator::new(
            coordinator::ExecutionMode::DistributedTrusted,
        );
        let peer = NodeId(Ulid::new());
        c.admit_peer(
            peer,
            CrowdTier::Trusted,
            verification::PeerCapability::default(),
        )
        .await
        .unwrap();
        c.evict_peer(peer).await.unwrap();
        assert_eq!(c.reputation.len(), 0);
    }

    #[tokio::test]
    async fn test_coordinator_execution_mode() {
        let c = coordinator::DefaultCrowdCoordinator::new(coordinator::ExecutionMode::Local);
        assert!(matches!(
            c.execution_mode(),
            coordinator::ExecutionMode::Local
        ));
    }
}
