// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Determinism Toggle (F-SCHED-05)
//!
//! ## Purpose
//! Reproducible mode for testing. Fixed seed + deterministic reductions.
//! Costs ~15% throughput.
//!
//! ## What This File Does
//! - `DeterminismConfig` — enabled flag + seed
//! - `set_determinism(config)` — sets the global determinism configuration
//!
//! ## Dependencies
//! - `serde` — for config serialization

use serde::{Deserialize, Serialize};

/// Determinism configuration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DeterminismConfig {
    /// Whether determinism mode is enabled.
    pub enabled: bool,
    /// The fixed random seed.
    pub seed: u64,
}

impl Default for DeterminismConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            seed: 0,
        }
    }
}

/// Set the global determinism configuration.
///
/// ## Parameters
/// - `config`: The determinism config (enabled + seed)
///
/// ## Side Effects
/// When enabled, the inference engine will:
/// - Use the fixed seed for all random operations
/// - Use deterministic reductions (no atomics)
/// - Accept ~15% throughput cost
pub fn set_determinism(_config: DeterminismConfig) {
    // Phase 8: stub. The real implementation will set a global AtomicU64
    // and the inference engine will check it before each generation.
}
