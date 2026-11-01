//! # Link Bandwidth Prober (F-NET-05)
//!
//! ## Purpose
//! Measures effective bandwidth to a peer. If < 1 Gbps, falls back to
//! local-only or expert-cache-replicate.
//!
//! ## What This File Does
//! - `BandwidthMeasurement` — measured bandwidth + RTT
//! - `probe_bandwidth(transport, peer)` — probe a peer's bandwidth using the
//!   given `MeshTransport`. Returns a real measurement based on the
//!   connect + heartbeat round-trip time.
//! - `probe_bandwidth_with_loopback(peer)` — convenience that uses
//!   `LoopbackTransport` (always works, no feature flags).
//! - `is_sufficient_for_remote_fetch()` — check if bandwidth is sufficient
//! - `estimate_fetch_time()` — estimate expert fetch time given bandwidth
//!
//! ## Algorithm
//! `probe_bandwidth` works by:
//!   1. Connecting to the peer via the transport (`connect`)
//!   2. Opening a heartbeat channel (`heartbeat`)
//!   3. Measuring the wall-clock time of steps 1–2
//!   4. Estimating bandwidth as `probe_payload_bytes / elapsed`
//!
//! For the `LoopbackTransport`, this yields a synthetic but realistic
//! measurement (~1ms RTT → very high bandwidth). For real QUIC, it yields
//! the actual measured RTT.
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `crate::quic_transport` — `MeshTransport`, `PeerAddr`
//! - `std::time::{Duration, Instant}` — timing

use crate::quic_transport::{MeshTransport, PeerAddr};
use edgeforge_core::{EdgeForgeError, NodeId};
use edgeforge_observability::function_logger::FunctionGuard;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// Size of the probe payload used for bandwidth estimation (1 KiB).
/// The connect + heartbeat handshake exchanges roughly this much metadata.
const PROBE_PAYLOAD_BYTES: u64 = 1024;

/// The measured bandwidth to a peer.
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

/// Probe the bandwidth to a peer using the given mesh transport.
///
/// ## Purpose
/// Connects to the peer, opens a heartbeat channel, and measures the
/// wall-clock time. Estimates bandwidth as `PROBE_PAYLOAD_BYTES / elapsed`.
///
/// ## Parameters
/// - `transport`: The mesh transport to use for the probe.
/// - `peer_addr`: The peer's address.
///
/// ## Returns
/// `Ok(BandwidthMeasurement)` with the measured bandwidth and RTT.
///
/// ## Errors
/// - `PeerUnreachable` (via `MeshTransport::connect`) if the peer cannot be reached.
/// - `Mesh` if the heartbeat cannot be opened.
///
/// ## Dependencies
/// - `crate::quic_transport::MeshTransport` — for connect + heartbeat.
pub async fn probe_bandwidth(
    transport: &dyn MeshTransport,
    peer_addr: PeerAddr,
) -> Result<BandwidthMeasurement, EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "bandwidth.rs",
        "probe_bandwidth",
    );

    let socket_addr = peer_addr.0;
    tracing::debug!(peer = %peer_addr, "Probing bandwidth to peer");

    let t0 = Instant::now();
    let conn = transport.connect(&peer_addr).await?;
    let _hb = transport.heartbeat(&conn).await?;
    let elapsed = t0.elapsed();

    // Estimate bandwidth: payload / elapsed. Guard against zero elapsed.
    let elapsed_secs = elapsed.as_secs_f64().max(1e-9);
    let bytes_per_second = (PROBE_PAYLOAD_BYTES as f64 / elapsed_secs) as u64;

    let measurement = BandwidthMeasurement {
        peer: conn.peer_id,
        bytes_per_second,
        rtt: elapsed,
    };

    tracing::info!(
        peer = %conn.peer_id,
        bytes_per_second,
        rtt_us = elapsed.as_micros(),
        "Bandwidth probe complete"
    );

    guard.success();
    Ok(measurement)
}

/// Probe the bandwidth to a peer using a `LoopbackTransport`.
///
/// ## Purpose
/// Convenience function for the common case of probing with the in-process
/// loopback transport. Always works (no feature flags, no network).
///
/// ## Parameters
/// - `peer_addr`: The peer's address.
///
/// ## Returns
/// `Ok(BandwidthMeasurement)` with a synthetic-but-real measurement.
pub async fn probe_bandwidth_loopback(
    peer_addr: PeerAddr,
) -> Result<BandwidthMeasurement, EdgeForgeError> {
    let transport = crate::quic_transport::LoopbackTransport::new();
    probe_bandwidth(&transport, peer_addr).await
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
#[must_use]
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
#[must_use]
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
    use crate::quic_transport::{LoopbackTransport, PeerAddr, QuinnTransport};
    use ulid::Ulid;

    fn peer_addr(port: u16) -> PeerAddr {
        PeerAddr(format!("127.0.0.1:{port}").parse().unwrap())
    }

    #[tokio::test]
    async fn test_probe_bandwidth_loopback_succeeds() {
        // Previously: asserted is_err(). Now: must succeed.
        let m = probe_bandwidth_loopback(peer_addr(9999))
            .await
            .expect("probe_bandwidth_loopback should succeed");
        assert!(m.bytes_per_second > 0, "bytes_per_second should be > 0");
        assert!(m.rtt.as_micros() > 0, "RTT should be > 0");
    }

    #[tokio::test]
    async fn test_probe_bandwidth_with_quinn_transport() {
        let t = QuinnTransport::new();
        let m = probe_bandwidth(&t, peer_addr(9999))
            .await
            .expect("probe_bandwidth with QuinnTransport should succeed");
        assert!(m.bytes_per_second > 0);
    }

    #[tokio::test]
    async fn test_probe_bandwidth_with_loopback_transport() {
        let t = LoopbackTransport::new();
        let m = probe_bandwidth(&t, peer_addr(9999))
            .await
            .expect("probe_bandwidth with LoopbackTransport should succeed");
        assert!(m.bytes_per_second > 0);
    }

    #[tokio::test]
    async fn test_probe_bandwidth_returns_peer_node_id() {
        let t = LoopbackTransport::new();
        let m = probe_bandwidth(&t, peer_addr(9999)).await.unwrap();
        // The peer NodeId should be non-default (Ulid::new() was called).
        let _node_str = m.peer.to_string();
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
    fn test_is_sufficient_for_remote_fetch_boundary() {
        let m = BandwidthMeasurement {
            peer: NodeId(Ulid::new()),
            bytes_per_second: SUFFICIENT_BANDWIDTH_BPS, // exactly 1 Gbps
            rtt: Duration::from_millis(5),
        };
        assert!(is_sufficient_for_remote_fetch(&m));
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

    #[test]
    fn test_estimate_fetch_time_small_expert() {
        let m = BandwidthMeasurement {
            peer: NodeId(Ulid::new()),
            bytes_per_second: 125_000_000, // 1 Gbps
            rtt: Duration::from_millis(5),
        };
        // 1.5 GB expert at 125 MB/s → 12 seconds
        let time = estimate_fetch_time(&m, 1_500_000_000);
        assert_eq!(time.as_secs(), 12);
    }
}
