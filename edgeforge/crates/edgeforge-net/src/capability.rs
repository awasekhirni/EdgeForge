// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Peer Capability Descriptor (G-03 fix)
//!
//! ## Purpose
//! Describes a peer's hardware capabilities and currently-hosted experts.
//! Broadcast in every heartbeat so the crowd coordinator and streaming graph
//! can make informed routing decisions.
//!
//! ## What This File Does
//! - `PeerCapability` — hardware + hosted experts + thermal + bandwidth + region
//! - `expert_capacity()` — how many experts the peer can host
//! - `can_host_expert()` — whether the peer can host an additional expert
//!
//! ## Dependencies
//! - `edgeforge-core` — ExpertId, LayerIdx
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `serde` — serialization

use edgeforge_core::{ExpertId, LayerIdx};
use edgeforge_observability::function_logger::FunctionGuard;
use serde::{Deserialize, Serialize};

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// A peer's hardware capability + currently-hosted experts.
///
/// ## Purpose
/// Broadcast in every heartbeat so the crowd coordinator and the streaming
/// graph can make informed routing decisions.
///
/// ## Fields
/// - `gpu_model`: GPU model name (e.g., "RTX 3090")
/// - `vram_bytes`: Total VRAM in bytes
/// - `ram_bytes`: Total system RAM in bytes
/// - `ssd_gib`: SSD capacity in GiB
/// - `hosted_experts`: List of (layer, expert) pairs currently resident
/// - `thermal_celsius`: Current SSD temperature
/// - `bandwidth_bps`: Network bandwidth in bits per second
/// - `region`: Grid region for carbon-aware routing (ADR-026)
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
            bandwidth_bps: 1_000_000_000, // 1 Gbps default
            region: "WORLD-AVG".to_string(),
        }
    }
}

impl PeerCapability {
    /// How many experts this peer can host given its VRAM.
    ///
    /// ## Purpose
    /// Calculates the maximum number of Q4_K_M experts (~1.5 GB each) the
    /// peer can host in VRAM.
    ///
    /// ## Returns
    /// `u32` — the maximum number of experts.
    ///
    /// ## Formula
    /// `vram_bytes / 1_500_000_000` (1.5 GB per Q4_K_M expert)
    pub fn expert_capacity(&self) -> u32 {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-net", "capability.rs", "expert_capacity");
        if self.vram_bytes == 0 {
            return 0;
        }
        (self.vram_bytes / 1_500_000_000) as u32
    }

    /// Whether the peer can host an additional expert.
    ///
    /// ## Purpose
    /// Checks if the peer has room for one more expert given its current
    /// hosted experts and VRAM capacity.
    ///
    /// ## Returns
    /// `true` if the peer can host another expert.
    pub fn can_host_expert(&self) -> bool {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-net", "capability.rs", "can_host_expert");
        self.hosted_experts.len() < self.expert_capacity() as usize
    }

    /// Returns the number of currently hosted experts.
    pub fn hosted_count(&self) -> usize {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "capability.rs", "hosted_count");
        self.hosted_experts.len()
    }

    /// Returns whether the peer has a GPU.
    pub fn has_gpu(&self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "capability.rs", "has_gpu");
        self.gpu_model.is_some() && self.vram_bytes > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expert_capacity_rtx_3090() {
        let cap = PeerCapability {
            gpu_model: Some("RTX 3090".to_string()),
            vram_bytes: 24_000_000_000, // 24 GB
            ..Default::default()
        };
        // 24 GB / 1.5 GB = 16 experts
        assert_eq!(cap.expert_capacity(), 16);
    }

    #[test]
    fn test_expert_capacity_no_vram() {
        let cap = PeerCapability::default();
        assert_eq!(cap.expert_capacity(), 0);
    }

    #[test]
    fn test_can_host_expert() {
        let cap = PeerCapability {
            vram_bytes: 3_000_000_000, // 3 GB → 2 experts
            ..Default::default()
        };
        assert!(cap.can_host_expert()); // 0 < 2
    }

    #[test]
    fn test_cannot_host_expert_when_full() {
        let cap = PeerCapability {
            vram_bytes: 3_000_000_000, // 2 experts max
            hosted_experts: vec![(LayerIdx(0), ExpertId(0)), (LayerIdx(0), ExpertId(1))],
            ..Default::default()
        };
        assert!(!cap.can_host_expert()); // 2 == 2
    }

    #[test]
    fn test_has_gpu() {
        let cap = PeerCapability {
            gpu_model: Some("RTX 4090".to_string()),
            vram_bytes: 24_000_000_000,
            ..Default::default()
        };
        assert!(cap.has_gpu());
    }

    #[test]
    fn test_no_gpu() {
        let cap = PeerCapability::default();
        assert!(!cap.has_gpu());
    }
}
