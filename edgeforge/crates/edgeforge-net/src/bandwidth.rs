// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Link Bandwidth Prober (F-NET-05)
//!
//! ## Purpose
//! Measures effective bandwidth to a peer. If < 1 Gbps, falls back to
//! local-only or expert-cache-replicate.
//!
//! ## What This File Does
//! - `BandwidthMeasurement` — measured bandwidth + RTT
//! - `probe_bandwidth()` — probe a peer's bandwidth
//! - `is_sufficient_for_remote_fetch()` — check if bandwidth is sufficient
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `std::time::Duration` — RTT measurement

use edgeforge_core::{EdgeForgeError, NodeId};
use edgeforge_observability::function_logger::FunctionGuard;
use std::time::Duration;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// The measured bandwidth to a peer.
///
/// ## Fields
/// - `peer`: The peer's node ID
/// - `bytes_per_second`: Effective bandwidth in bytes per second
/// - `rtt`: Round-trip time
#[derive(Clone, Copy, Debug)]
pub struct BandwidthMeasurement {
    /// The peer's node ID.
    pub peer: NodeId,
    /// Effective bandwidth in bytes per second.
    pub bytes_per_second: u64,
    /// Round-trip time.
    pub rtt: Duration,
}

/// Threshold for sufficient bandwidth: 1 Gbps = 125 MB/s = 125,000,000 B/s.
pub const SUFFICIENT_BANDWIDTH_BPS: u64 = 125_000_000;

/// Probe the bandwidth to a peer.
///
/// ## Purpose
/// Sends a small probe payload over the QUIC mesh and measures the round-trip
/// time. Estimates bandwidth based on the payload size and RTT.
///
/// ## Parameters
/// - `peer`: The peer's node ID.
///
/// ## Returns
/// `Ok(BandwidthMeasurement)` with the measured bandwidth and RTT.
///
/// ## Errors
/// - `NotImplemented` — requires the quic-transport feature.
/// - `PeerUnreachable` if the peer cannot be reached.
///
/// ## Dependencies
/// - `crate::quic_transport::MeshTransport` — for sending the probe.
pub async fn probe_bandwidth(peer: NodeId) -> Result<BandwidthMeasurement, EdgeForgeError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "bandwidth.rs", "probe_bandwidth");
    tracing::debug!(node = %peer, "Probing bandwidth to peer");
    let err = EdgeForgeError::NotImplemented("probe_bandwidth — enable quic-transport feature");
    guard.error(&err.to_string());
    Err(err)
}

/// Is the measured bandwidth sufficient for distributed expert fetch?
///
/// ## Purpose
/// Checks if the measured bandwidth meets the 1 Gbps threshold for
/// distributed expert fetching. If not, the system falls back to
/// local-only or expert-cache-replicate.
///
/// ## Parameters
/// - `m`: The bandwidth measurement.
///
/// ## Returns
/// `true` if the bandwidth is sufficient (≥ 1 Gbps).
pub fn is_sufficient_for_remote_fetch(m: &BandwidthMeasurement) -> bool {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "bandwidth.rs",
        "is_sufficient_for_remote_fetch",
    );
    m.bytes_per_second >= SUFFICIENT_BANDWIDTH_BPS
}

/// Estimate the time to fetch an expert over the measured bandwidth.
///
/// ## Purpose
/// Calculates the estimated fetch time for an expert tensor given the
/// measured bandwidth.
///
/// ## Parameters
/// - `m`: The bandwidth measurement.
/// - `expert_bytes`: The size of the expert tensor in bytes.
///
/// ## Returns
/// `Duration` — the estimated fetch time.
pub fn estimate_fetch_time(m: &BandwidthMeasurement, expert_bytes: u64) -> Duration {
    let _guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "bandwidth.rs",
        "estimate_fetch_time",
    );
    if m.bytes_per_second == 0 {
        return Duration::from_secs(u64::MAX);
    }
    Duration::from_micros((expert_bytes * 1_000_000) / m.bytes_per_second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    #[tokio::test]
    async fn test_probe_bandwidth_not_implemented() {
        let result = probe_bandwidth(NodeId(Ulid::new())).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_is_sufficient_for_remote_fetch_true() {
        let m = BandwidthMeasurement {
            peer: NodeId(Ulid::new()),
            bytes_per_second: 200_000_000, // 1.6 Gbps
            rtt: Duration::from_millis(5),
        };
        assert!(is_sufficient_for_remote_fetch(&m));
    }

    #[test]
    fn test_is_sufficient_for_remote_fetch_false() {
        let m = BandwidthMeasurement {
            peer: NodeId(Ulid::new()),
            bytes_per_second: 50_000_000, // 400 Mbps — below 1 Gbps
            rtt: Duration::from_millis(20),
        };
        assert!(!is_sufficient_for_remote_fetch(&m));
    }

    #[test]
    fn test_estimate_fetch_time() {
        let m = BandwidthMeasurement {
            peer: NodeId(Ulid::new()),
            bytes_per_second: 1_000_000_000, // 1 GB/s
            rtt: Duration::from_millis(5),
        };
        // 1.5 GB expert → 1.5 seconds
        let time = estimate_fetch_time(&m, 1_500_000_000);
        assert_eq!(time.as_secs(), 1);
    }

    #[test]
    fn test_estimate_fetch_time_zero_bandwidth() {
        let m = BandwidthMeasurement {
            peer: NodeId(Ulid::new()),
            bytes_per_second: 0,
            rtt: Duration::from_millis(5),
        };
        let time = estimate_fetch_time(&m, 1_000_000);
        assert_eq!(time.as_secs(), u64::MAX);
    }
}
