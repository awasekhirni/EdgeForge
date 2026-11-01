// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Verification Policies and Outcomes (ADR-017, ADR-018)
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
//!
//! ## Dependencies
//! - `edgeforge_core::{ExpertId, LayerIdx}` — for hosted_experts
//! - `serde` — serialization
//!
//! ## Output
//! `VerificationOutcome` — surfaced via the on_token callback and
//! X-EdgeForge-Verification SSE header.

use edgeforge_core::{ExpertId, LayerIdx};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `CrowdTier` enumeration — see module documentation for variants.
pub enum CrowdTier {
    Trusted,
    Cooperative,
    Open,
}
#[derive(Clone, Debug)]
/// `VerificationPolicy` — see module documentation for purpose and usage.
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
/// `VerificationSampler` enumeration — see module documentation for variants.
pub enum VerificationSampler {
    None,
    ProofOfQuality { sample_rate: f32 },
    Optimistic { recheck_rate: f32 },
}
#[derive(Clone, Debug)]
/// `RedundancyPolicy` — see module documentation for purpose and usage.
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
/// `ChurnPolicy` — see module documentation for purpose and usage.
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
/// `VerificationOutcome` enumeration — see module documentation for variants.
pub enum VerificationOutcome {
    NotRequired,
    Verified,
    Pending,
    Corrected,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `PeerCapability` — see module documentation for purpose and usage.
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
