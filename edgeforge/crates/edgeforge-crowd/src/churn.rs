//! # Churn-Resilient Scheduler (ADR-017)
//!
//! ## Purpose
//! Tier 3 (Open Crowd) assumes continuous churn. This module provides the
//! policy + scheduler that reassigns expert fetches on peer loss.
//!
//! ## What This File Does
//! - `ChurnPolicy` — reassignment timeout + heartbeat miss threshold
//! - `ChurnResilientScheduler` — k-of-n redundant execution + timeout reassignment
//! - `replication_factor()` — how many peers to send each request to
//! - `reassignment_timeout_ms()` — timeout before reassigning
//!
//! ## Dependencies
//! - `edgeforge-crowd` — CrowdTier
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `serde` — serialization
//!
//! ## Output
//! `u8` from replication_factor() — the k-of-n redundancy level.
//!
//! ## Function Logging
//! `timestamp - edgeforge-crowd - churn.rs - function - STATUS`

use crate::CrowdTier;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Churn tolerance policy.
///
/// ## Purpose
/// Configures the churn-resilient scheduler for Tier 3 (Open Crowd).
/// Controls reassignment timeout and heartbeat miss threshold.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ChurnPolicy {
    /// Reassign an expert to another peer after this many ms without response.
    pub reassignment_timeout_ms: u32,
    /// A peer is marked dead after this many consecutive missed heartbeats.
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

/// The churn-resilient scheduler.
///
/// ## Purpose
/// Sends the same expert request to N peers (k-of-n redundancy) and
/// accepts the first valid response. On timeout, reassigns to another peer.
pub struct ChurnResilientScheduler {
    /// The churn policy.
    pub policy: ChurnPolicy,
    /// The crowd tier (determines redundancy level).
    pub tier: CrowdTier,
    /// The redundancy level (k-of-n).
    pub redundancy: u8,
}

impl ChurnResilientScheduler {
    /// Creates a new churn-resilient scheduler for the given tier.
    ///
    /// ## Purpose
    /// Initializes the scheduler with the appropriate redundancy level
    /// based on the crowd tier:
    /// - Tier 1 (Trusted): 1 (no redundancy)
    /// - Tier 2 (Cooperative): 2 (send to 2 peers, accept first)
    /// - Tier 3 (Open): 3 (send to 3 peers, accept first)
    ///
    /// ## Parameters
    /// - `tier`: The crowd tier.
    /// - `policy`: The churn policy.
    ///
    /// ## Returns
    /// A new `ChurnResilientScheduler` instance.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-crowd - churn.rs - ChurnResilientScheduler::new - STATUS`
    pub fn new(tier: CrowdTier, policy: ChurnPolicy) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "churn.rs",
            "ChurnResilientScheduler::new",
        );
        let redundancy = match tier {
            CrowdTier::Trusted => 1,
            CrowdTier::Cooperative => 2,
            CrowdTier::Open => 3,
        };
        Self {
            policy,
            tier,
            redundancy,
        }
    }

    /// How many peers to send each expert request to.
    ///
    /// ## Returns
    /// `u8` — the replication factor (1 for Tier 1, 2 for Tier 2, 3 for Tier 3).
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-crowd - churn.rs - replication_factor - STATUS`
    pub fn replication_factor(&self) -> u8 {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "churn.rs",
            "replication_factor",
        );
        self.redundancy
    }

    /// After how many ms should we reassign if no response?
    ///
    /// ## Returns
    /// `u32` — the reassignment timeout in milliseconds.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-crowd - churn.rs - reassignment_timeout_ms - STATUS`
    pub fn reassignment_timeout_ms(&self) -> u32 {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "churn.rs",
            "reassignment_timeout_ms",
        );
        self.policy.reassignment_timeout_ms
    }

    /// Returns the heartbeat miss threshold.
    ///
    /// ## Returns
    /// `u8` — the number of consecutive missed heartbeats before declaring a peer dead.
    pub fn heartbeat_miss_threshold(&self) -> u8 {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "churn.rs",
            "heartbeat_miss_threshold",
        );
        self.policy.heartbeat_miss_threshold
    }

    /// Returns the crowd tier.
    pub fn tier(&self) -> CrowdTier {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-crowd",
            "churn.rs",
            "tier",
        );
        self.tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trusted_redundancy_1() {
        let s = ChurnResilientScheduler::new(CrowdTier::Trusted, ChurnPolicy::default());
        assert_eq!(s.replication_factor(), 1);
    }

    #[test]
    fn test_cooperative_redundancy_2() {
        let s = ChurnResilientScheduler::new(CrowdTier::Cooperative, ChurnPolicy::default());
        assert_eq!(s.replication_factor(), 2);
    }

    #[test]
    fn test_open_redundancy_3() {
        let s = ChurnResilientScheduler::new(CrowdTier::Open, ChurnPolicy::default());
        assert_eq!(s.replication_factor(), 3);
    }

    #[test]
    fn test_reassignment_timeout() {
        let policy = ChurnPolicy {
            reassignment_timeout_ms: 200,
            heartbeat_miss_threshold: 5,
        };
        let s = ChurnResilientScheduler::new(CrowdTier::Open, policy);
        assert_eq!(s.reassignment_timeout_ms(), 200);
        assert_eq!(s.heartbeat_miss_threshold(), 5);
    }

    #[test]
    fn test_tier() {
        let s = ChurnResilientScheduler::new(CrowdTier::Cooperative, ChurnPolicy::default());
        assert_eq!(s.tier(), CrowdTier::Cooperative);
    }

    #[test]
    fn test_default_policy() {
        let p = ChurnPolicy::default();
        assert_eq!(p.reassignment_timeout_ms, 100);
        assert_eq!(p.heartbeat_miss_threshold, 3);
    }
}
