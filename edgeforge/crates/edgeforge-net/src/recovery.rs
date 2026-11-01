// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Heartbeat-Driven Recovery (F-NET-06)
//!
//! ## Purpose
//! Heartbeat-based peer liveness monitoring. On timeout, marks the peer's
//! experts as remote-fetch-failed and re-pages from local SSD or an alternate peer.
//!
//! ## What This File Does
//! - `HeartbeatConfig` — interval, timeout, miss threshold
//! - `Heartbeat` — per-peer heartbeat state
//! - `Heartbeat::on_received()` — record a received heartbeat
//! - `Heartbeat::on_missed()` — record a missed heartbeat; returns true if peer should die
//! - `repage_from_ssd()` — re-page an expert from local SSD after peer loss
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `std::time::Duration` — heartbeat intervals

use edgeforge_core::EdgeForgeError;
use edgeforge_observability::function_logger::FunctionGuard;
use std::time::Duration;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// Heartbeat configuration.
///
/// ## Purpose
/// Configures the heartbeat interval, timeout, and miss threshold for
/// disconnect-aware recovery.
#[derive(Clone, Copy, Debug)]
pub struct HeartbeatConfig {
    /// How often to send heartbeats.
    pub interval: Duration,
    /// How long to wait before considering a heartbeat missed.
    pub timeout: Duration,
    /// Number of consecutive missed heartbeats before declaring peer dead.
    pub miss_threshold: u8,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            timeout: Duration::from_secs(15),
            miss_threshold: 3,
        }
    }
}

/// Per-peer heartbeat state.
///
/// ## Purpose
/// Tracks the number of missed heartbeats for a single peer. When the
/// miss count reaches the threshold, the peer is declared dead.
pub struct Heartbeat {
    /// The heartbeat configuration.
    pub config: HeartbeatConfig,
    /// Number of consecutive missed heartbeats.
    pub missed_count: u8,
    /// Timestamp of the last received heartbeat (Unix ms).
    pub last_received_ms: i64,
    /// Whether the peer is alive.
    pub alive: bool,
}

impl Heartbeat {
    /// Creates a new heartbeat tracker with the given configuration.
    ///
    /// ## Parameters
    /// - `config`: Heartbeat configuration.
    ///
    /// ## Returns
    /// A new `Heartbeat` instance with the peer initially alive.
    pub fn new(config: HeartbeatConfig) -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "Heartbeat::new");
        Self {
            config,
            missed_count: 0,
            last_received_ms: 0,
            alive: true,
        }
    }

    /// Record a received heartbeat.
    ///
    /// ## Purpose
    /// Resets the missed heartbeat counter and marks the peer as alive.
    ///
    /// ## Parameters
    /// - `timestamp_ms`: The timestamp of the received heartbeat (Unix ms).
    pub fn on_received(&mut self, timestamp_ms: i64) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "on_received");
        self.missed_count = 0;
        self.last_received_ms = timestamp_ms;
        self.alive = true;
        tracing::trace!(timestamp_ms, "Heartbeat received");
    }

    /// Record a missed heartbeat.
    ///
    /// ## Purpose
    /// Increments the missed heartbeat counter. If the counter reaches the
    /// threshold, the peer is declared dead.
    ///
    /// ## Returns
    /// `true` if the peer should be considered dead (miss threshold reached).
    pub fn on_missed(&mut self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "on_missed");
        self.missed_count += 1;
        tracing::warn!(
            missed = self.missed_count,
            threshold = self.config.miss_threshold,
            "Heartbeat missed"
        );
        if self.missed_count >= self.config.miss_threshold {
            self.alive = false;
            tracing::warn!(
                "Peer declared dead after {} missed heartbeats",
                self.missed_count
            );
            return true;
        }
        false
    }

    /// Returns whether the peer is alive.
    pub fn is_alive(&self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "is_alive");
        self.alive
    }

    /// Returns the number of consecutive missed heartbeats.
    pub fn missed_count(&self) -> u8 {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "missed_count");
        self.missed_count
    }

    /// Returns the time since the last received heartbeat.
    ///
    /// ## Parameters
    /// - `now_ms`: Current timestamp (Unix ms).
    ///
    /// ## Returns
    /// `Duration` since the last heartbeat, or `Duration::MAX` if never received.
    pub fn time_since_last(&self, now_ms: i64) -> Duration {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "time_since_last");
        if self.last_received_ms == 0 {
            return Duration::MAX;
        }
        Duration::from_millis((now_ms - self.last_received_ms).max(0) as u64)
    }
}

/// Re-page an expert from local SSD after a peer is lost.
///
/// ## Purpose
/// When a peer is declared dead, its experts need to be re-paged from local
/// SSD or an alternate peer. This function triggers the re-paging.
///
/// ## Parameters
/// - `expert_id`: The expert to re-page (e.g., "layer_3_expert_5").
///
/// ## Returns
/// `Ok(())` on success.
///
/// ## Errors
/// - `NotImplemented` — requires the weight pager to be wired.
/// - `Io` if the SSD read fails.
///
/// ## Dependencies
/// - `edgeforge_storage::WeightPager` — for SSD reads (called by orchestration).
pub async fn repage_from_ssd(expert_id: &str) -> Result<(), EdgeForgeError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "repage_from_ssd");
    tracing::info!(
        expert = expert_id,
        "Re-paging expert from local SSD after peer loss"
    );
    let err = EdgeForgeError::NotImplemented("repage_from_ssd — requires weight pager wiring");
    guard.error(&err.to_string());
    Err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_received_resets_counter() {
        let mut hb = Heartbeat::new(HeartbeatConfig::default());
        hb.on_missed();
        hb.on_missed();
        assert_eq!(hb.missed_count(), 2);
        hb.on_received(1000);
        assert_eq!(hb.missed_count(), 0);
        assert!(hb.is_alive());
    }

    #[test]
    fn test_heartbeat_missed_reaches_threshold() {
        let mut hb = Heartbeat::new(HeartbeatConfig {
            miss_threshold: 3,
            ..Default::default()
        });
        assert!(!hb.on_missed()); // 1
        assert!(!hb.on_missed()); // 2
        assert!(hb.on_missed()); // 3 → dead
        assert!(!hb.is_alive());
    }

    #[test]
    fn test_heartbeat_alive_initially() {
        let hb = Heartbeat::new(HeartbeatConfig::default());
        assert!(hb.is_alive());
        assert_eq!(hb.missed_count(), 0);
    }

    #[test]
    fn test_time_since_last_never_received() {
        let hb = Heartbeat::new(HeartbeatConfig::default());
        assert_eq!(hb.time_since_last(1000), Duration::MAX);
    }

    #[test]
    fn test_time_since_last_received() {
        let mut hb = Heartbeat::new(HeartbeatConfig::default());
        hb.on_received(1000);
        let elapsed = hb.time_since_last(1500);
        assert_eq!(elapsed.as_millis(), 500);
    }

    #[tokio::test]
    async fn test_repage_from_ssd_not_implemented() {
        let result = repage_from_ssd("layer_3_expert_5").await;
        assert!(result.is_err());
    }
}
