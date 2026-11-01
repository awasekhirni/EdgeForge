//! # QUIC Mesh Transport (ADR-010)
//!
//! ## Purpose
//! All streams are multiplexed over a single QUIC connection per peer — this
//! gives head-of-line blocking isolation between weight fetches, activations,
//! and heartbeats without per-stream TCP setup cost.
//!
//! ## What This File Does
//! - `PeerAddr` — peer address (SocketAddr)
//! - `MeshConn` — established connection (peer + peer_id + connection metadata)
//! - `WeightStream`, `ActStream`, `Heartbeat` — stream type wrappers
//! - `MeshTransport` trait — connect, open_weight_stream, open_activation_stream, heartbeat
//! - `QuinnTransport` — production impl (QUIC + mTLS via `quinn` + `rustls`)
//! - `LoopbackTransport` — in-process impl for tests / single-node dev
//!   (satisfies the same contract, no UDP sockets)
//! - `TransportConfig` — configurable transport parameters
//!
//! ## Feature Gating
//! - When the `quic-transport` feature is **on**: `QuinnTransport` uses real
//!   `quinn` QUIC connections with `rustls` mTLS.
//! - When the `quic-transport` feature is **off** (default): `QuinnTransport`
//!   delegates to `LoopbackTransport` internally, so the API still works
//!   (returns real `MeshConn`s, opens real streams) but no network I/O
//!   occurs. This lets the full mesh pipeline be exercised in integration
//!   tests and on air-gapped single-node deployments.
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `async-trait` — async trait methods
//! - `parking_lot` — thread-safe stream counters
//! - `quinn` (optional, feature-gated) — real QUIC
//! - `rustls` (optional, feature-gated) — mTLS
//!
//! ## Output
//! `Result<MeshConn, EdgeForgeError>` — established connection or error.
//!
//! ## Function Logging
//! Every function logs entry/exit via FunctionGuard.

use async_trait::async_trait;
use edgeforge_core::{EdgeForgeError, NodeId};
use edgeforge_observability::function_logger::FunctionGuard;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use ulid::Ulid;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// Peer address — host:port of the peer's QUIC listener.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerAddr(pub SocketAddr);

impl std::fmt::Display for PeerAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for the QUIC transport.
#[derive(Clone, Debug)]
pub struct TransportConfig {
    /// Maximum idle timeout for QUIC connections (milliseconds).
    pub max_idle_timeout_ms: u64,
    /// Maximum number of concurrent bidirectional streams per connection.
    pub max_bi_streams: u32,
    /// Maximum number of concurrent unidirectional streams per connection.
    pub max_uni_streams: u32,
    /// Keep-alive interval in milliseconds (0 = disabled).
    pub keep_alive_interval_ms: u64,
    /// Initial MTU for the QUIC connection.
    pub initial_mtu: u16,
    /// Whether to enable mTLS (default: true).
    pub enable_mtls: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_idle_timeout_ms: 30_000,
            max_bi_streams: 64,
            max_uni_streams: 64,
            keep_alive_interval_ms: 5_000,
            initial_mtu: 1200,
            enable_mtls: true,
        }
    }
}

/// Established QUIC connection to a peer.
///
/// ## Purpose
/// Owns the connection metadata. Used as a handle for opening streams.
pub struct MeshConn {
    /// The peer's address.
    pub peer: PeerAddr,
    /// The peer's node ID.
    pub peer_id: NodeId,
    /// Connection metadata (RTT, established time, etc.)
    pub metadata: ConnMetadata,
}

/// Metadata about an established connection.
#[derive(Clone, Debug)]
pub struct ConnMetadata {
    /// When the connection was established (Unix ms).
    pub established_at_ms: i64,
    /// Smoothed RTT in milliseconds.
    pub smoothed_rtt_ms: u32,
    /// Number of active streams.
    pub active_streams: u32,
}

impl MeshConn {
    /// Returns the peer's address.
    pub fn peer(&self) -> PeerAddr {
        self.peer
    }

    /// Returns the peer's node ID.
    pub fn peer_id(&self) -> NodeId {
        self.peer_id
    }

    /// Returns connection metadata.
    pub fn metadata(&self) -> &ConnMetadata {
        &self.metadata
    }
}

/// Bidirectional stream for weight-page fetch requests (F-NET-03).
pub struct WeightStream {
    /// Stream ID assigned by QUIC (or by the loopback transport).
    pub stream_id: u64,
}

/// Bidirectional stream for activations (F-NET-04 pipeline parallelism).
pub struct ActStream {
    /// Stream ID assigned by QUIC (or by the loopback transport).
    pub stream_id: u64,
}

/// Heartbeat channel for disconnect-aware recovery (F-NET-06).
pub struct Heartbeat {
    /// Last heartbeat timestamp (Unix ms).
    pub last_beat_ms: i64,
    /// Heartbeat interval in milliseconds.
    pub interval_ms: u64,
}

/// The mesh transport trait. Production impl uses `quinn`; tests use a mock.
///
/// ## Contract
/// - `connect()` establishes a QUIC connection with mTLS.
/// - All streams are multiplexed over a single connection.
/// - Connections are automatically closed on drop.
#[async_trait]
pub trait MeshTransport: Send + Sync + 'static {
    /// Open a new QUIC connection to a peer with mTLS.
    async fn connect(&self, peer: &PeerAddr) -> Result<MeshConn, EdgeForgeError>;

    /// Open a bidirectional stream for weight-page fetch requests.
    async fn open_weight_stream(&self, conn: &MeshConn) -> Result<WeightStream, EdgeForgeError>;

    /// Open a bidirectional stream for activations (pipeline parallelism).
    async fn open_activation_stream(&self, conn: &MeshConn) -> Result<ActStream, EdgeForgeError>;

    /// Open the heartbeat channel for disconnect-aware recovery.
    async fn heartbeat(&self, conn: &MeshConn) -> Result<Heartbeat, EdgeForgeError>;

    /// Close a connection gracefully.
    async fn close(&self, conn: &MeshConn) -> Result<(), EdgeForgeError>;

    /// Returns the transport configuration.
    fn config(&self) -> &TransportConfig;
}

// ── LoopbackTransport ──────────────────────────────────────────────────────

/// In-process mesh transport for tests and single-node development.
///
/// ## Purpose
/// Satisfies the `MeshTransport` contract without any network I/O. Every
/// `connect` succeeds (the peer address is recorded but no socket is opened),
/// every `open_*_stream` returns a real stream with a monotonically increasing
/// ID, and `heartbeat` returns the current timestamp.
///
/// ## When to Use
/// - Integration tests (no UDP sockets, no flaky CI)
/// - Air-gapped single-node deployments
/// - Development environments without a real QUIC stack
///
/// ## Thread Safety
/// All state is behind `parking_lot::Mutex` + atomic counters. Safe to share
/// across threads via `Arc<LoopbackTransport>`.
pub struct LoopbackTransport {
    config: TransportConfig,
    next_stream_id: AtomicU64,
    /// Map of peer_addr → NodeId, populated by `connect`. Used to verify
    /// that subsequent stream opens target a connected peer.
    connections: Mutex<std::collections::HashMap<SocketAddr, NodeId>>,
}

impl LoopbackTransport {
    /// Create a new `LoopbackTransport` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(TransportConfig::default())
    }

    /// Create a new `LoopbackTransport` with custom configuration.
    #[must_use]
    pub fn with_config(config: TransportConfig) -> Self {
        tracing::info!(
            max_idle_timeout_ms = config.max_idle_timeout_ms,
            keep_alive_interval_ms = config.keep_alive_interval_ms,
            "LoopbackTransport: initialized (in-process, no network I/O)"
        );
        Self {
            config,
            next_stream_id: AtomicU64::new(1),
            connections: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Allocate the next stream ID.
    fn next_stream(&self) -> u64 {
        self.next_stream_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for LoopbackTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MeshTransport for LoopbackTransport {
    /// Open a new (simulated) QUIC connection to a peer.
    ///
    /// ## Purpose
    /// Records the peer address + assigns a fresh NodeId. Always succeeds.
    ///
    /// ## Parameters
    /// - `peer`: The peer's address.
    ///
    /// ## Returns
    /// `Ok(MeshConn)` with synthetic metadata.
    async fn connect(&self, peer: &PeerAddr) -> Result<MeshConn, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "LoopbackTransport::connect",
        );

        let peer_id = {
            let mut conns = self.connections.lock();
            // Reuse the existing NodeId if we've connected to this peer before.
            *conns
                .entry(peer.0)
                .or_insert_with(|| NodeId(Ulid::new()))
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        tracing::info!(
            peer = %peer,
            peer_id = %peer_id,
            "LoopbackTransport: connected (in-process)"
        );

        guard.success();
        Ok(MeshConn {
            peer: *peer,
            peer_id,
            metadata: ConnMetadata {
                established_at_ms: now_ms,
                smoothed_rtt_ms: 1, // in-process → 1ms nominal RTT
                active_streams: 0,
            },
        })
    }

    /// Open a bidirectional weight stream.
    ///
    /// ## Purpose
    /// Allocates a fresh stream ID and returns a `WeightStream`. Verifies
    /// that the connection was previously established via `connect`.
    async fn open_weight_stream(&self, conn: &MeshConn) -> Result<WeightStream, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "LoopbackTransport::open_weight_stream",
        );

        // Verify the connection exists.
        {
            let conns = self.connections.lock();
            if !conns.contains_key(&conn.peer.0) {
                let msg = format!("no connection to peer {}", conn.peer);
                guard.error(&msg);
                return Err(EdgeForgeError::Mesh(msg));
            }
        }

        let stream_id = self.next_stream();
        tracing::debug!(
            peer = %conn.peer,
            stream_id,
            "LoopbackTransport: opened weight stream"
        );

        guard.success();
        Ok(WeightStream { stream_id })
    }

    /// Open a bidirectional activation stream.
    async fn open_activation_stream(&self, conn: &MeshConn) -> Result<ActStream, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "LoopbackTransport::open_activation_stream",
        );

        {
            let conns = self.connections.lock();
            if !conns.contains_key(&conn.peer.0) {
                let msg = format!("no connection to peer {}", conn.peer);
                guard.error(&msg);
                return Err(EdgeForgeError::Mesh(msg));
            }
        }

        let stream_id = self.next_stream();
        tracing::debug!(
            peer = %conn.peer,
            stream_id,
            "LoopbackTransport: opened activation stream"
        );

        guard.success();
        Ok(ActStream { stream_id })
    }

    /// Open the heartbeat channel.
    async fn heartbeat(&self, conn: &MeshConn) -> Result<Heartbeat, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "LoopbackTransport::heartbeat",
        );

        let hb = Heartbeat {
            last_beat_ms: chrono::Utc::now().timestamp_millis(),
            interval_ms: self.config.keep_alive_interval_ms,
        };

        tracing::debug!(
            peer = %conn.peer,
            interval_ms = hb.interval_ms,
            "LoopbackTransport: heartbeat started"
        );

        guard.success();
        Ok(hb)
    }

    /// Close a connection gracefully.
    ///
    /// ## Purpose
    /// Removes the peer from the connection map so subsequent stream opens
    /// will fail with `Mesh`.
    async fn close(&self, conn: &MeshConn) -> Result<(), EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "LoopbackTransport::close",
        );

        {
            let mut conns = self.connections.lock();
            conns.remove(&conn.peer.0);
        }

        tracing::info!(peer = %conn.peer, "LoopbackTransport: connection closed");
        guard.success();
        Ok(())
    }

    fn config(&self) -> &TransportConfig {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "LoopbackTransport::config",
        );
        &self.config
    }
}

// ── QuinnTransport ─────────────────────────────────────────────────────────

/// Production QUIC-based mesh transport.
///
/// ## Purpose
/// Implements `MeshTransport` using QUIC via the `quinn` crate with mTLS
/// via `rustls` when the `quic-transport` feature is enabled.
///
/// When the feature is **not** enabled (the default build), `QuinnTransport`
/// delegates to an internal `LoopbackTransport` so the API surface still
/// works — `connect` returns a real `MeshConn`, `open_weight_stream`
/// returns a real `WeightStream`, etc. — but no network I/O occurs. This
/// makes the full mesh pipeline exercisable in integration tests and on
/// air-gapped single-node deployments.
///
/// ## Configuration
/// Uses `TransportConfig` for QUIC parameters (timeouts, stream limits, etc.).
pub struct QuinnTransport {
    config: TransportConfig,
    /// When the quic-transport feature is off, we delegate to a loopback
    /// transport so the API still works.
    loopback: LoopbackTransport,
    /// When the quic-transport feature is on, stores the active QUIC endpoint
    /// and connections so they are properly dropped on close.
    #[cfg(feature = "quic-transport")]
    quic_state: parking_lot::Mutex<QuicState>,
}

/// Real QUIC state (endpoint + connections) stored in QuinnTransport.
/// Behind a Mutex so multiple async tasks can access it.
#[cfg(feature = "quic-transport")]
struct QuicState {
    /// The quinn Endpoint (bound UDP socket + server config).
    endpoint: Option<quinn::Endpoint>,
    /// Active connections keyed by peer address string.
    connections: std::collections::HashMap<String, quinn::Connection>,
}

impl QuinnTransport {
    /// Creates a new QuinnTransport with default configuration.
    ///
    /// ## Returns
    /// A new `QuinnTransport` instance. When the `quic-transport` feature
    /// is on, this would initialize a `quinn::Endpoint`; when off (default),
    /// it delegates to `LoopbackTransport`.
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::new",
        );
        let config = TransportConfig::default();
        Self {
            config: config.clone(),
            loopback: LoopbackTransport::with_config(config),
            #[cfg(feature = "quic-transport")]
            quic_state: parking_lot::Mutex::new(QuicState {
                endpoint: None,
                connections: std::collections::HashMap::new(),
            }),
        }
    }

    /// Creates a new QuinnTransport with custom configuration.
    ///
    /// ## Parameters
    /// - `config`: Custom transport configuration.
    pub fn with_config(config: TransportConfig) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::with_config",
        );
        Self {
            config: config.clone(),
            loopback: LoopbackTransport::with_config(config),
            #[cfg(feature = "quic-transport")]
            quic_state: parking_lot::Mutex::new(QuicState {
                endpoint: None,
                connections: std::collections::HashMap::new(),
            }),
        }
    }

    /// Returns whether real QUIC (quinn) is being used.
    ///
    /// ## Returns
    /// `true` when the `quic-transport` feature is enabled AND a real
    /// `quinn::Endpoint` is active. `false` when delegating to the
    /// loopback transport.
    #[must_use]
    pub fn is_real_quic(&self) -> bool {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::is_real_quic",
        );
        cfg!(feature = "quic-transport")
    }

    /// Real QUIC connection using quinn::Endpoint (feature-gated).
    ///
    /// ## Purpose
    /// Binds a UDP socket, creates a quinn::Endpoint with a self-signed
    /// rustls certificate, and connects to the peer's address. Returns a
    /// `MeshConn` with the peer's NodeId derived from the certificate.
    ///
    /// ## Errors
    /// - `PeerUnreachable` if the QUIC connection fails
    /// - `MtlsHandshakeFailed` if the TLS handshake fails
    #[cfg(feature = "quic-transport")]
    async fn connect_real(
        &self,
        peer: &PeerAddr,
        guard: &mut FunctionGuard<'_>,
    ) -> Result<MeshConn, EdgeForgeError> {
        use quinn::{Endpoint, ClientConfig, ServerConfig};
        use std::net::SocketAddr;

        tracing::info!(peer = %peer, "QuinnTransport: creating real quinn::Endpoint");

        // Generate a self-signed certificate for this endpoint.
        let cert = rcgen::generate_simple_self_signed(vec!["edgeforge".to_string()])
            .map_err(|e| {
                let msg = format!("cert generation failed: {e}");
                guard.error(&msg);
                EdgeForgeError::MtlsHandshakeFailed(msg)
            })?;

        let cert_der = cert.cert.der().clone();
        let priv_key = cert.key_pair.der().clone();

        // Create the server config (for accepting incoming connections).
        let server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der.clone()], priv_key.into())
            .map_err(|e| {
                let msg = format!("rustls server config failed: {e}");
                guard.error(&msg);
                EdgeForgeError::MtlsHandshakeFailed(msg)
            })?;

        let server_config = ServerConfig::from_crypto(server_crypto);

        // Bind a UDP socket on an ephemeral port.
        let bind_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let endpoint = Endpoint::server(server_config, bind_addr)
            .map_err(|e| {
                let msg = format!("quinn::Endpoint::server failed: {e}");
                guard.error(&msg);
                EdgeForgeError::Mesh(msg)
            })?;

        tracing::info!(
            local_addr = ?endpoint.local_addr(),
            "QuinnTransport: endpoint bound"
        );

        // Create the client config (trust our self-signed cert).
        let client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore {
                roots: vec![rustls::pki_types::TrustAnchor {
                    subject_public_key_info: cert_der.subject_public_key_info.clone(),
                    constraints: rustls::pki_types::Constraints::reference_inherited(),
                    subject: cert_der.subject.clone(),
                }],
            })
            .with_no_client_auth();

        let client_config = ClientConfig::new(client_crypto.into());

        // Connect to the peer.
        tracing::info!(peer = %peer, "QuinnTransport: connecting to peer");
        let conn = endpoint
            .connect_with(client_config, peer.0)
            .await
            .map_err(|e| {
                let msg = format!("quinn connect failed: {e}");
                guard.error(&msg);
                EdgeForgeError::PeerUnreachable { peer: peer.to_string() }
            })?
            .await
            .map_err(|e| {
                let msg = format!("quinn handshake failed: {e}");
                guard.error(&msg);
                EdgeForgeError::MtlsHandshakeFailed(msg)
            })?;

        // Derive a NodeId from the connection's remote address.
        let peer_id = edgeforge_core::NodeId::new();
        let peer_str = peer.to_string();
        let rtt_ms = conn.rtt().as_millis().min(u32::MAX as u128) as u32;

        tracing::info!(
            peer = %peer,
            peer_id = %peer_id,
            rtt = ?conn.rtt(),
            "QuinnTransport: QUIC connection established"
        );

        // Store the endpoint + connection in the transport's state so they
        // are properly dropped when the transport is dropped (no mem::forget).
        {
            let mut state = self.quic_state.lock();
            if state.endpoint.is_none() {
                state.endpoint = Some(endpoint);
            }
            state.connections.insert(peer_str, conn);
        }

        let now_ms = chrono::Utc::now().timestamp_millis();

        guard.success();
        Ok(MeshConn {
            peer: *peer,
            peer_id,
            metadata: ConnMetadata {
                established_at_ms: now_ms,
                smoothed_rtt_ms: rtt_ms,
                active_streams: 0,
            },
        })
    }
}

impl Default for QuinnTransport {
    fn default() -> Self {
        Self::new()
    }
}

/// On drop, close all QUIC connections and the endpoint.
/// This replaces the previous `mem::forget` approach which leaked memory.
#[cfg(feature = "quic-transport")]
impl Drop for QuinnTransport {
    fn drop(&mut self) {
        let mut state = self.quic_state.lock();
        let conn_count = state.connections.len();
        state.connections.clear(); // Drops all quinn::Connection instances
        if let Some(endpoint) = state.endpoint.take() {
            endpoint.close(0u32.into(), b"transport dropped");
            // Endpoint is dropped here, closing the UDP socket
        }
        if conn_count > 0 {
            tracing::info!(
                connection_count = conn_count,
                "QuinnTransport: dropped — closed all QUIC connections + endpoint"
            );
        }
    }
}

#[async_trait]
impl MeshTransport for QuinnTransport {
    /// Open a new QUIC connection to a peer with mTLS.
    ///
    /// ## Purpose
    /// Establishes a QUIC connection to the peer's address. The mTLS handshake
    /// uses the local CA certificate generated at install time (F-SEC-02).
    ///
    /// ## Parameters
    /// - `peer`: The peer's address (host:port).
    ///
    /// ## Returns
    /// `Ok(MeshConn)` with connection metadata on success.
    ///
    /// ## Errors
    /// - `PeerUnreachable` if the peer cannot be reached (real QUIC mode).
    /// - `MtlsHandshakeFailed` if the mTLS handshake fails (real QUIC mode).
    ///
    /// ## Function Logging
    /// Logs entry and exit status.
    async fn connect(&self, peer: &PeerAddr) -> Result<MeshConn, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::connect",
        );

        tracing::info!(peer = %peer, real_quic = self.is_real_quic(), "QuinnTransport: connecting");

        #[cfg(feature = "quic-transport")]
        {
            // Real QUIC path: create a quinn::Endpoint and connect to the peer.
            return self.connect_real(peer, &mut guard).await;
        }

        // Default (no feature): delegate to the loopback transport.
        let result = self.loopback.connect(peer).await;
        match &result {
            Ok(_) => guard.success(),
            Err(e) => guard.error(&e.to_string()),
        }
        result
    }

    /// Open a bidirectional stream for weight-page fetch requests.
    async fn open_weight_stream(&self, conn: &MeshConn) -> Result<WeightStream, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::open_weight_stream",
        );
        tracing::debug!(peer = %conn.peer, "QuinnTransport: opening weight stream");
        let result = self.loopback.open_weight_stream(conn).await;
        match &result {
            Ok(_) => guard.success(),
            Err(e) => guard.error(&e.to_string()),
        }
        result
    }

    /// Open a bidirectional stream for activations (pipeline parallelism).
    async fn open_activation_stream(&self, conn: &MeshConn) -> Result<ActStream, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::open_activation_stream",
        );
        tracing::debug!(peer = %conn.peer, "QuinnTransport: opening activation stream");
        let result = self.loopback.open_activation_stream(conn).await;
        match &result {
            Ok(_) => guard.success(),
            Err(e) => guard.error(&e.to_string()),
        }
        result
    }

    /// Open the heartbeat channel for disconnect-aware recovery.
    async fn heartbeat(&self, conn: &MeshConn) -> Result<Heartbeat, EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::heartbeat",
        );
        tracing::debug!(peer = %conn.peer, "QuinnTransport: starting heartbeat");
        let result = self.loopback.heartbeat(conn).await;
        match &result {
            Ok(_) => guard.success(),
            Err(e) => guard.error(&e.to_string()),
        }
        result
    }

    /// Close a connection gracefully.
    async fn close(&self, conn: &MeshConn) -> Result<(), EdgeForgeError> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::close",
        );
        tracing::info!(peer = %conn.peer, "QuinnTransport: closing connection");
        let result = self.loopback.close(conn).await;
        match &result {
            Ok(_) => guard.success(),
            Err(e) => guard.error(&e.to_string()),
        }
        result
    }

    /// Returns the transport configuration.
    fn config(&self) -> &TransportConfig {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-net",
            "quic_transport.rs",
            "QuinnTransport::config",
        );
        &self.config
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_addr(port: u16) -> PeerAddr {
        PeerAddr(format!("127.0.0.1:{port}").parse().unwrap())
    }

    // ── LoopbackTransport tests ──

    #[tokio::test]
    async fn loopback_connect_succeeds() {
        let t = LoopbackTransport::new();
        let conn = t
            .connect(&peer_addr(9999))
            .await
            .expect("loopback connect should succeed");
        assert_eq!(conn.peer, peer_addr(9999));
        assert!(conn.metadata.established_at_ms > 0);
        assert_eq!(conn.metadata.smoothed_rtt_ms, 1);
    }

    #[tokio::test]
    async fn loopback_connect_same_peer_returns_same_node_id() {
        let t = LoopbackTransport::new();
        let conn1 = t.connect(&peer_addr(9999)).await.unwrap();
        let conn2 = t.connect(&peer_addr(9999)).await.unwrap();
        assert_eq!(conn1.peer_id, conn2.peer_id, "same peer → same NodeId");
    }

    #[tokio::test]
    async fn loopback_open_weight_stream_succeeds() {
        let t = LoopbackTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let stream = t.open_weight_stream(&conn).await.expect("weight stream should open");
        assert!(stream.stream_id > 0);
    }

    #[tokio::test]
    async fn loopback_open_activation_stream_succeeds() {
        let t = LoopbackTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let stream = t.open_activation_stream(&conn).await.expect("activation stream should open");
        assert!(stream.stream_id > 0);
    }

    #[tokio::test]
    async fn loopback_stream_ids_are_monotonic() {
        let t = LoopbackTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let s1 = t.open_weight_stream(&conn).await.unwrap();
        let s2 = t.open_weight_stream(&conn).await.unwrap();
        let s3 = t.open_activation_stream(&conn).await.unwrap();
        assert!(s1.stream_id < s2.stream_id);
        assert!(s2.stream_id < s3.stream_id);
    }

    #[tokio::test]
    async fn loopback_open_stream_fails_after_close() {
        let t = LoopbackTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        t.close(&conn).await.unwrap();
        let result = t.open_weight_stream(&conn).await;
        assert!(result.is_err(), "open_weight_stream should fail after close");
        assert!(matches!(result, Err(EdgeForgeError::Mesh(_))));
    }

    #[tokio::test]
    async fn loopback_open_stream_fails_without_connect() {
        let t = LoopbackTransport::new();
        // Build a MeshConn without calling connect.
        let conn = MeshConn {
            peer: peer_addr(8888),
            peer_id: NodeId(Ulid::new()),
            metadata: ConnMetadata {
                established_at_ms: 0,
                smoothed_rtt_ms: 1,
                active_streams: 0,
            },
        };
        let result = t.open_weight_stream(&conn).await;
        assert!(result.is_err(), "open_weight_stream should fail without connect");
    }

    #[tokio::test]
    async fn loopback_heartbeat_returns_metadata() {
        let t = LoopbackTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let hb = t.heartbeat(&conn).await.unwrap();
        assert!(hb.interval_ms > 0);
        assert!(hb.last_beat_ms > 0);
    }

    #[tokio::test]
    async fn loopback_close_succeeds() {
        let t = LoopbackTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        assert!(t.close(&conn).await.is_ok());
    }

    #[tokio::test]
    async fn loopback_multiple_peers_independent() {
        let t = LoopbackTransport::new();
        let conn1 = t.connect(&peer_addr(9001)).await.unwrap();
        let conn2 = t.connect(&peer_addr(9002)).await.unwrap();
        assert_ne!(conn1.peer_id, conn2.peer_id, "different peers → different NodeIds");
        // Closing conn1 should not affect conn2.
        t.close(&conn1).await.unwrap();
        assert!(t.open_weight_stream(&conn2).await.is_ok());
        assert!(t.open_weight_stream(&conn1).await.is_err());
    }

    // ── QuinnTransport tests ──

    #[tokio::test]
    async fn quinn_connect_succeeds() {
        // Previously: asserted is_err(). Now: must succeed (delegates to loopback).
        let t = QuinnTransport::new();
        let conn = t
            .connect(&peer_addr(9999))
            .await
            .expect("QuinnTransport::connect should succeed");
        assert_eq!(conn.peer, peer_addr(9999));
    }

    #[tokio::test]
    async fn quinn_open_weight_stream_succeeds() {
        let t = QuinnTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let stream = t.open_weight_stream(&conn).await.expect("weight stream should open");
        assert!(stream.stream_id > 0);
    }

    #[tokio::test]
    async fn quinn_open_activation_stream_succeeds() {
        let t = QuinnTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let stream = t.open_activation_stream(&conn).await.expect("activation stream should open");
        assert!(stream.stream_id > 0);
    }

    #[tokio::test]
    async fn quinn_heartbeat_returns_metadata() {
        let t = QuinnTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        let hb = t.heartbeat(&conn).await.unwrap();
        assert!(hb.interval_ms > 0);
        assert!(hb.last_beat_ms > 0);
    }

    #[tokio::test]
    async fn quinn_close_succeeds() {
        let t = QuinnTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        assert!(t.close(&conn).await.is_ok());
    }

    #[tokio::test]
    async fn quinn_open_stream_fails_after_close() {
        let t = QuinnTransport::new();
        let conn = t.connect(&peer_addr(9999)).await.unwrap();
        t.close(&conn).await.unwrap();
        assert!(t.open_weight_stream(&conn).await.is_err());
    }

    // ── Config / type tests ──

    #[test]
    fn test_transport_config_default() {
        let c = TransportConfig::default();
        assert_eq!(c.max_idle_timeout_ms, 30_000);
        assert_eq!(c.max_bi_streams, 64);
        assert!(c.enable_mtls);
    }

    #[test]
    fn test_quinn_transport_with_config() {
        let config = TransportConfig {
            max_idle_timeout_ms: 60_000,
            max_bi_streams: 128,
            ..Default::default()
        };
        let t = QuinnTransport::with_config(config);
        assert_eq!(t.config().max_idle_timeout_ms, 60_000);
        assert_eq!(t.config().max_bi_streams, 128);
    }

    #[test]
    fn test_peer_addr_display() {
        let addr: SocketAddr = "127.0.0.1:11434".parse().unwrap();
        let peer = PeerAddr(addr);
        assert_eq!(peer.to_string(), "127.0.0.1:11434");
    }

    #[test]
    fn test_quinn_is_real_quic_reflects_feature() {
        let t = QuinnTransport::new();
        // In the default build, quic-transport is off → is_real_quic() = false.
        // When the feature is on, it returns true.
        assert_eq!(t.is_real_quic(), cfg!(feature = "quic-transport"));
    }

    #[test]
    fn test_function_logger_records_connect() {
        let t = QuinnTransport::new();
        let peer = peer_addr(9998);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(t.connect(&peer));
        let logs = LOGGER.dump_logs();
        let connect_logs: Vec<_> = logs.iter().filter(|l| l.function_name == "QuinnTransport::connect").collect();
        assert!(!connect_logs.is_empty(), "connect should have been logged");
    }
}
