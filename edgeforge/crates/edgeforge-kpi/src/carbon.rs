// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
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
pub trait CarbonIntensitySource: Send + Sync + 'static {
    async fn grams_per_kwh(&self, region: &str, at: i64) -> Option<f64>;
}
/// `StaticCarbonTable` — see module documentation for purpose and usage.
pub struct StaticCarbonTable;
#[async_trait]
impl CarbonIntensitySource for StaticCarbonTable {
    async fn grams_per_kwh(&self, region: &str, _: i64) -> Option<f64> {
        Some(match region {
            "US-WA" | "US-OR" | "EU-NORDIC" => 50.0,
            "US-CA" => 150.0,
            "EU-AVG" => 250.0,
            "US-TX" => 400.0,
            "CN-AVG" => 530.0,
            "IN-AVG" => 700.0,
            _ => 475.0,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `CarbonEstimate` — see module documentation for purpose and usage.
pub struct CarbonEstimate {
    pub energy_joules: f64,
    pub grams_co2e: f64,
    pub region: String,
    pub grams_per_kwh: f64,
}
impl CarbonEstimate {
    pub fn from_energy(j: f64, g: f64, r: &str) -> Self {
        let kwh = j / 3_600_000.0;
        Self {
            energy_joules: j,
            grams_co2e: kwh * g,
            region: r.into(),
            grams_per_kwh: g,
        }
    }
}
#[derive(Clone, Debug)]
/// `CarbonPeer` — see module documentation for purpose and usage.
pub struct CarbonPeer {
    pub region: String,
    pub energy_joules_per_token: f64,
    pub rtt_ms: u32,
}
/// `CarbonAwareRouter` — see module documentation for purpose and usage.
pub struct CarbonAwareRouter {
    pub carbon: Box<dyn CarbonIntensitySource>,
    pub latency_slo: Duration,
}
impl CarbonAwareRouter {
    /// Creates a new instance with default configuration.
    pub fn new(c: Box<dyn CarbonIntensitySource>, l: Duration) -> Self {
        Self {
            carbon: c,
            latency_slo: l,
        }
    }
    /// Route — see module documentation for details.
    pub async fn route<'a>(&self, peers: &'a [CarbonPeer], at: i64) -> Option<&'a CarbonPeer> {
        let mut best: Option<(f64, &CarbonPeer)> = None;
        for p in peers {
            if Duration::from_millis(u64::from(p.rtt_ms) * 2) > self.latency_slo {
                continue;
            }
            let g = self
                .carbon
                .grams_per_kwh(&p.region, at)
                .await
                .unwrap_or(475.0);
            let est = CarbonEstimate::from_energy(p.energy_joules_per_token, g, &p.region);
            match &best {
                None => best = Some((est.grams_co2e, p)),
                Some((b, _)) if est.grams_co2e < *b => best = Some((est.grams_co2e, p)),
                _ => {}
            }
        }
        best.map(|(_, p)| p)
    }
}
