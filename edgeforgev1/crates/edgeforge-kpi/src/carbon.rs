// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[async_trait]
pub trait CarbonIntensitySource: Send + Sync + 'static {
    async fn grams_per_kwh(&self, region: &str, at: i64) -> Option<f64>;
}
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
pub struct CarbonPeer {
    pub region: String,
    pub energy_joules_per_token: f64,
    pub rtt_ms: u32,
}
pub struct CarbonAwareRouter {
    pub carbon: Box<dyn CarbonIntensitySource>,
    pub latency_slo: Duration,
}
impl CarbonAwareRouter {
    pub fn new(c: Box<dyn CarbonIntensitySource>, l: Duration) -> Self {
        Self {
            carbon: c,
            latency_slo: l,
        }
    }
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
