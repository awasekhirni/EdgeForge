// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # KPI Tests — edgeforge-kpi
//!
//! Tests for: QPPO evaluation, traceability matrix, carbon-aware router.

#[cfg(test)]
mod tests {
    use edgeforge_kpi::*;
    use std::time::Duration;

    // ── QPPO Tests ──────────────────────────────────────────────────

    #[test]
    fn test_qppo_kani_coverage_below_lcl() {
        let set = qppo::QppoSet::default();
        let v = set.evaluate(qppo::QppoId::KaniCoverage, 85.0, 0);
        assert!(v.is_some());
        assert_eq!(v.unwrap().status, qppo::QppoStatus::BelowLcl);
    }

    #[test]
    fn test_qppo_build_time_above_ucl() {
        let set = qppo::QppoSet::default();
        let v = set.evaluate(qppo::QppoId::BuildTimeP95, 45.0, 0);
        assert!(v.is_some());
        assert_eq!(v.unwrap().status, qppo::QppoStatus::AboveUcl);
    }

    #[test]
    fn test_qppo_in_control() {
        let set = qppo::QppoSet::default();
        let v = set.evaluate(qppo::QppoId::KaniCoverage, 95.0, 0);
        assert!(v.is_none()); // In control → no violation
    }

    // ── Traceability Tests ─────────────────────────────────────────

    #[test]
    fn test_traceability_add_and_query() {
        let mut m = traceability::TraceabilityMatrix::new();
        m.add(
            "edgeforge_core::types::ModelId",
            traceability::ArchitecturalTrace {
                adr_ids: vec!["ADR-001".to_string()],
                requirement_ids: vec!["F-LOAD-01".to_string()],
                verification_ids: vec!["tests/core.rs".to_string()],
                cmmi_process_areas: vec![qppo::ProcessAreaId::Ts],
            },
        );
        let items = m.items_for_adr("ADR-001");
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_traceability_to_json() {
        let mut m = traceability::TraceabilityMatrix::new();
        m.add("test::item", traceability::ArchitecturalTrace::default());
        let json = m.to_json();
        assert!(json.contains("test::item"));
    }

    // ── Carbon Router Tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_carbon_router_picks_lowest_carbon() {
        let router = carbon::CarbonAwareRouter::new(
            Box::new(carbon::StaticCarbonTable),
            Duration::from_millis(500),
        );
        let peers = vec![
            carbon::CarbonPeer {
                region: "US-TX".into(),
                energy_joules_per_token: 100.0,
                rtt_ms: 50,
            },
            carbon::CarbonPeer {
                region: "EU-NORDIC".into(),
                energy_joules_per_token: 100.0,
                rtt_ms: 80,
            },
        ];
        let chosen = router.route(&peers, 0).await.unwrap();
        assert_eq!(chosen.region, "EU-NORDIC"); // 50 gCO2e/kWh vs 400
    }

    #[tokio::test]
    async fn test_carbon_router_skips_high_latency() {
        let router = carbon::CarbonAwareRouter::new(
            Box::new(carbon::StaticCarbonTable),
            Duration::from_millis(100), // tight SLO
        );
        let peers = vec![
            carbon::CarbonPeer {
                region: "EU-NORDIC".into(),
                energy_joules_per_token: 100.0,
                rtt_ms: 80,
            },
            // 2×80 = 160ms > 100ms SLO → skipped
            carbon::CarbonPeer {
                region: "US-TX".into(),
                energy_joules_per_token: 100.0,
                rtt_ms: 40,
            },
            // 2×40 = 80ms ≤ 100ms → candidate
        ];
        let chosen = router.route(&peers, 0).await.unwrap();
        assert_eq!(chosen.region, "US-TX");
    }

    #[tokio::test]
    async fn test_carbon_router_returns_none_when_all_violate_slo() {
        let router = carbon::CarbonAwareRouter::new(
            Box::new(carbon::StaticCarbonTable),
            Duration::from_millis(10),
        );
        let peers = vec![carbon::CarbonPeer {
            region: "US-TX".into(),
            energy_joules_per_token: 100.0,
            rtt_ms: 50,
        }];
        assert!(router.route(&peers, 0).await.is_none());
    }
}
