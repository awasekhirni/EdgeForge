//! # Carbon-Aware Inference Routing (ADR-026)
//!
//! ## Purpose
//! Routes expert fetches to the lowest-carbon peer that satisfies the latency
//! SLO. Uses a static IEA 2024 carbon table for air-gapped deployments; opt-in
//! API for connected deployments.
//!
//! ## What This File Does
//! - `CarbonIntensitySource` trait — `grams_per_kwh(region, at) -> Option<f64>`
//! - `StaticCarbonTable` — IEA 2024 world-average grid intensities
//! - `CarbonEstimate` — energy_joules, grams_co2e, region, grams_per_kwh
//! - `CarbonPeer` — region, energy_joules_per_token, rtt_ms
//! - `CarbonAwareRouter` — routes to lowest-carbon peer satisfying latency SLO
//! - `route(peers, at)` — select the best peer
//!
//! ## Dependencies
//! - `async-trait` — for CarbonIntensitySource
//! - `serde` — for CarbonEstimate
//! - `std::time::Duration` — for latency SLO
//!
//! ## Output
//! `Option<&CarbonPeer>` — the lowest-carbon peer, or None if all violate the SLO.
//!
//! ## Static Carbon Table (IEA 2024)
//! | Region | gCO2e/kWh |
//! |---|---|
//! | US-WA / US-OR / EU-NORDIC | 50 |
//! | US-CA | 150 |
//! | EU-AVG | 250 |
//! | US-TX | 400 |
//! | CN-AVG | 530 |
//! | IN-AVG | 700 |
//! | WORLD-AVG | 475 |
//!
//! ## Function: route
//! - **Purpose:** Route to the lowest-carbon peer satisfying the latency SLO
//! - **Parameters:** `&[CarbonPeer]`, `at: i64` (timestamp)
//! - **Returns:** `Option<&CarbonPeer>` — best peer or None
//! - **Algorithm:** For each peer: check 2×RTT ≤ SLO, query carbon intensity,
//!   compute grams CO2e = (energy_joules / 3.6M) × grams_per_kwh. Select minimum.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[async_trait]
/// The `CarbonIntensitySource` trait — see module documentation for the contract.
pub trait CarbonIntensitySource: Send + Sync + 'static { async fn grams_per_kwh(&self, region: &str, at: i64) -> Option<f64>; }
/// `StaticCarbonTable` — see module documentation for purpose and usage.
pub struct StaticCarbonTable;
#[async_trait]
impl CarbonIntensitySource for StaticCarbonTable { async fn grams_per_kwh(&self, region: &str, _: i64) -> Option<f64> { Some(match region { "US-WA"|"US-OR"|"EU-NORDIC" => 50.0, "US-CA" => 150.0, "EU-AVG" => 250.0, "US-TX" => 400.0, "CN-AVG" => 530.0, "IN-AVG" => 700.0, _ => 475.0 }) } }
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `CarbonEstimate` — see module documentation for purpose and usage.
pub struct CarbonEstimate { pub energy_joules: f64, pub grams_co2e: f64, pub region: String, pub grams_per_kwh: f64 }
impl CarbonEstimate { pub fn from_energy(j: f64, g: f64, r: &str) -> Self { let kwh = j / 3_600_000.0; Self { energy_joules: j, grams_co2e: kwh * g, region: r.into(), grams_per_kwh: g } } }
#[derive(Clone, Debug)]
/// `CarbonPeer` — see module documentation for purpose and usage.
pub struct CarbonPeer { pub region: String, pub energy_joules_per_token: f64, pub rtt_ms: u32 }
/// `CarbonAwareRouter` — see module documentation for purpose and usage.
pub struct CarbonAwareRouter { pub carbon: Box<dyn CarbonIntensitySource>, pub latency_slo: Duration }
impl CarbonAwareRouter {
    /// Creates a new instance with default configuration.
    pub fn new(c: Box<dyn CarbonIntensitySource>, l: Duration) -> Self { Self { carbon: c, latency_slo: l } }
    /// Route — see module documentation for details.
    pub async fn route<'a>(&self, peers: &'a [CarbonPeer], at: i64) -> Option<&'a CarbonPeer> {
        let mut best: Option<(f64, &CarbonPeer)> = None;
        for p in peers { if Duration::from_millis(u64::from(p.rtt_ms)*2) > self.latency_slo { continue; } let g = self.carbon.grams_per_kwh(&p.region, at).await.unwrap_or(475.0); let est = CarbonEstimate::from_energy(p.energy_joules_per_token, g, &p.region); match &best { None => best = Some((est.grams_co2e, p)), Some((b, _)) if est.grams_co2e < *b => best = Some((est.grams_co2e, p)), _ => {} } }
        best.map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_static_carbon_table_known_regions() {
        let table = StaticCarbonTable;
        assert_eq!(table.grams_per_kwh("US-WA", 0).await, Some(50.0));
        assert_eq!(table.grams_per_kwh("EU-NORDIC", 0).await, Some(50.0));
        assert_eq!(table.grams_per_kwh("US-CA", 0).await, Some(150.0));
        assert_eq!(table.grams_per_kwh("EU-AVG", 0).await, Some(250.0));
        assert_eq!(table.grams_per_kwh("US-TX", 0).await, Some(400.0));
        assert_eq!(table.grams_per_kwh("CN-AVG", 0).await, Some(530.0));
        assert_eq!(table.grams_per_kwh("IN-AVG", 0).await, Some(700.0));
    }

    #[tokio::test]
    async fn test_static_carbon_table_unknown_region() {
        let table = StaticCarbonTable;
        assert_eq!(table.grams_per_kwh("UNKNOWN", 0).await, Some(475.0));
    }

    #[test]
    fn test_carbon_estimate_from_energy() {
        let est = CarbonEstimate::from_energy(3_600_000.0, 400.0, "US-TX");
        // 3.6M joules = 1 kWh × 400 g/kWh = 400 g
        assert!((est.grams_co2e - 400.0).abs() < 0.01);
        assert_eq!(est.region, "US-TX");
        assert_eq!(est.grams_per_kwh, 400.0);
    }

    #[tokio::test]
    async fn test_router_picks_lowest_carbon() {
        let router = CarbonAwareRouter::new(Box::new(StaticCarbonTable), Duration::from_millis(500));
        let peers = vec![
            CarbonPeer { region: "US-TX".into(), energy_joules_per_token: 100.0, rtt_ms: 50 },
            CarbonPeer { region: "EU-NORDIC".into(), energy_joules_per_token: 100.0, rtt_ms: 80 },
        ];
        let chosen = router.route(&peers, 0).await.unwrap();
        assert_eq!(chosen.region, "EU-NORDIC");
    }

    #[tokio::test]
    async fn test_router_skips_high_latency() {
        let router = CarbonAwareRouter::new(Box::new(StaticCarbonTable), Duration::from_millis(100));
        let peers = vec![
            CarbonPeer { region: "EU-NORDIC".into(), energy_joules_per_token: 100.0, rtt_ms: 80 },
            CarbonPeer { region: "US-TX".into(), energy_joules_per_token: 100.0, rtt_ms: 40 },
        ];
        let chosen = router.route(&peers, 0).await.unwrap();
        assert_eq!(chosen.region, "US-TX");
    }

    #[tokio::test]
    async fn test_router_returns_none_when_all_violate_slo() {
        let router = CarbonAwareRouter::new(Box::new(StaticCarbonTable), Duration::from_millis(10));
        let peers = vec![
            CarbonPeer { region: "US-TX".into(), energy_joules_per_token: 100.0, rtt_ms: 50 },
        ];
        assert!(router.route(&peers, 0).await.is_none());
    }

    #[tokio::test]
    async fn test_router_empty_peers_returns_none() {
        let router = CarbonAwareRouter::new(Box::new(StaticCarbonTable), Duration::from_millis(500));
        let peers: Vec<CarbonPeer> = vec![];
        assert!(router.route(&peers, 0).await.is_none());
    }

    #[tokio::test]
    async fn test_router_single_peer_within_slo() {
        let router = CarbonAwareRouter::new(Box::new(StaticCarbonTable), Duration::from_millis(500));
        let peers = vec![
            CarbonPeer { region: "US-CA".into(), energy_joules_per_token: 50.0, rtt_ms: 30 },
        ];
        let chosen = router.route(&peers, 0).await.unwrap();
        assert_eq!(chosen.region, "US-CA");
    }
}
