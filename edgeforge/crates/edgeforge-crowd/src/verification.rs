// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::{ExpertId, LayerIdx};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrowdTier {
    Trusted,
    Cooperative,
    Open,
}
#[derive(Clone, Debug)]
pub struct VerificationPolicy {
    pub deadline_ms: u32,
    pub sampler: VerificationSampler,
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
#[derive(Clone, Copy, Debug)]
pub enum VerificationSampler {
    None,
    ProofOfQuality { sample_rate: f32 },
    Optimistic { recheck_rate: f32 },
}
#[derive(Clone, Debug)]
pub struct RedundancyPolicy {
    pub replication_factor: u8,
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
#[derive(Clone, Debug)]
pub struct ChurnPolicy {
    pub reassignment_timeout_ms: u32,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationOutcome {
    NotRequired,
    Verified,
    Pending,
    Corrected,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerCapability {
    pub gpu_model: Option<String>,
    pub vram_bytes: u64,
    pub ram_bytes: u64,
    pub ssd_gib: u64,
    pub hosted_experts: Vec<(LayerIdx, ExpertId)>,
    pub thermal_celsius: f32,
    pub bandwidth_bps: u64,
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
