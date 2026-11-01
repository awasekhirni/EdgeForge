//! # mDNS Peer Discovery (F-NET-01)
//!
//! ## Purpose
//! Discovers peers on the local network without a central registry.
//! Uses the `_edgeforge._tcp` mDNS service name.
//!
//! ## What This File Does
//! - `DiscoveredPeer` — discovered peer info (node_id, hostname, addr, port)
//! - `MdnsDiscovery` — mDNS discovery service
//! - `advertise(port)` — advertise this node on the local network
//! - `discover()` — discover peers on the local network
//! - `stop()` — stop advertising
//!
//! ## Dependencies
//! - `edgeforge-core` — NodeId, EdgeForgeError
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `chrono` — timestamps
//!
//! ## Output
//! `Vec<DiscoveredPeer>` — list of discovered peers.
//!
//! ## Function Logging
//! `timestamp - edgeforge-net - discovery.rs - function - STATUS`

use edgeforge_core::{EdgeForgeError, NodeId};
use edgeforge_observability::function_logger::FunctionGuard;
use std::net::SocketAddr;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> = std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// A discovered peer on the local network.
///
/// ## Fields
/// - `node_id`: The peer's unique node identifier
/// - `hostname`: The peer's hostname
/// - `addr`: The peer's network address
/// - `port`: The peer's QUIC listener port
#[derive(Clone, Debug)]
pub struct DiscoveredPeer {
    /// The peer's unique node identifier.
    pub node_id: NodeId,
    /// The peer's hostname.
    pub hostname: String,
    /// The peer's network address.
    pub addr: SocketAddr,
    /// The peer's QUIC listener port.
    pub port: u16,
}

/// mDNS discovery service.
///
/// ## Purpose
/// Advertises this node on the local network via mDNS and discovers other
/// EdgeForge peers. Uses the `_edgeforge._tcp` service name.
///
/// ## Implementation Note
/// Phase 10: provides the API surface and logging. The real mDNS
/// implementation requires the `zeroconf` crate behind a feature flag
/// (it needs Avahi/Bonjour system libraries).
pub struct MdnsDiscovery {
    /// Whether this node is currently advertising.
    advertising: bool,
    /// The port being advertised.
    port: u16,
    /// Discovered peers (in-memory cache).
    discovered: Vec<DiscoveredPeer>,
}

impl MdnsDiscovery {
    /// Creates a new mDNS discovery service.
    ///
    /// ## Returns
    /// A new `MdnsDiscovery` instance.
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "MdnsDiscovery::new");
        Self {
            advertising: false,
            port: 0,
            discovered: Vec::new(),
        }
    }

    /// Start advertising this node on the local network.
    ///
    /// ## Purpose
    /// Registers a `_edgeforge._tcp` mDNS service so other peers can discover
    /// this node. The service is advertised on the specified port.
    ///
    /// ## Parameters
    /// - `port`: The QUIC listener port to advertise.
    ///
    /// ## Returns
    /// `Ok(())` on success.
    ///
    /// ## Errors
    /// - `Mesh` if the mDNS service cannot be registered.
    ///
    /// ## Dependencies
    /// - `zeroconf` crate (behind `mdns` feature flag) — for mDNS service registration.
    pub async fn advertise(&mut self, port: u16) -> Result<(), EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "advertise");
        tracing::info!(port, "MdnsDiscovery: advertising on _edgeforge._tcp");
        self.advertising = true;
        self.port = port;
        guard.success();
        Ok(())
    }

    /// Discover peers on the local network.
    ///
    /// ## Purpose
    /// Browses for `_edgeforge._tcp` mDNS services on the local network.
    /// Returns all discovered peers.
    ///
    /// ## Returns
    /// `Ok(Vec<DiscoveredPeer>)` — list of discovered peers.
    ///
    /// ## Errors
    /// - `Mesh` if mDNS browsing fails.
    ///
    /// ## Dependencies
    /// - `zeroconf` crate (behind `mdns` feature flag) — for mDNS service browsing.
    pub async fn discover(&mut self) -> Result<Vec<DiscoveredPeer>, EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "discover");
        tracing::debug!("MdnsDiscovery: browsing for _edgeforge._tcp services");
        // Phase 10: returns cached peers. Real implementation uses zeroconf.
        guard.success();
        Ok(self.discovered.clone())
    }

    /// Stop advertising this node.
    ///
    /// ## Purpose
    /// Unregisters the mDNS service so peers can no longer discover this node.
    ///
    /// ## Returns
    /// `Ok(())` on success.
    pub async fn stop(&mut self) -> Result<(), EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "stop");
        tracing::info!("MdnsDiscovery: stopping advertisement");
        self.advertising = false;
        guard.success();
        Ok(())
    }

    /// Returns whether this node is currently advertising.
    pub fn is_advertising(&self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "is_advertising");
        self.advertising
    }

    /// Returns the advertised port (0 if not advertising).
    pub fn port(&self) -> u16 {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "port");
        self.port
    }

    /// Add a manually discovered peer (for testing).
    pub fn add_peer(&mut self, peer: DiscoveredPeer) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "discovery.rs", "add_peer");
        self.discovered.push(peer);
    }
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    #[tokio::test]
    async fn test_advertise_and_stop() {
        let mut d = MdnsDiscovery::new();
        assert!(!d.is_advertising());
        d.advertise(11435).await.unwrap();
        assert!(d.is_advertising());
        assert_eq!(d.port(), 11435);
        d.stop().await.unwrap();
        assert!(!d.is_advertising());
    }

    #[tokio::test]
    async fn test_discover_returns_cached() {
        let mut d = MdnsDiscovery::new();
        d.add_peer(DiscoveredPeer {
            node_id: NodeId(Ulid::new()),
            hostname: "test-host".to_string(),
            addr: "127.0.0.1:11435".parse().unwrap(),
            port: 11435,
        });
        let peers = d.discover().await.unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].hostname, "test-host");
    }
}
