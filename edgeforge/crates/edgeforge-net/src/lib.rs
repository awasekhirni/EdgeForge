#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-net — L2 Networking (ADR-010 QUIC)
//!
//! ## Purpose
//! QUIC-based mesh transport with mTLS, mDNS peer discovery, bandwidth probing,
//! pipeline parallelism, and heartbeat-driven recovery.
//!
//! ## Modules
//! - `quic_transport` — `MeshTransport` trait + `QuinnTransport` (real QUIC
//!   when `quic-transport` feature is on) + `LoopbackTransport` (in-process,
//!   default). **No method returns `NotImplemented`** — the loopback transport
//!   satisfies the full contract without network I/O.
//! - `mesh` — `Mesh` coordinator + `MeshEvent`
//! - `discovery` — mDNS peer discovery (F-NET-01; in-memory cache by default,
//!   real `zeroconf` when `mdns` feature is on)
//! - `mtls` — Per-node Ed25519 cert generation + verification (F-SEC-02).
//!   **`LocalCa::generate`, `sign_node_cert`, `verify_peer_cert` all work
//!   without any feature flag** — they use `ed25519-dalek` + `ring`.
//! - `capability` — `PeerCapability` descriptor (G-03 fix)
//! - `pipeline` — Layer-stripe pipeline parallelism (F-NET-04).
//!   `forward_activations` uses `MeshTransport::open_activation_stream`.
//! - `bandwidth` — Link bandwidth prober (F-NET-05).
//!   `probe_bandwidth` uses `MeshTransport::connect` + `heartbeat`.
//! - `recovery` — Heartbeat-driven peer eviction + expert re-page (F-NET-06).
//!   `repage_from_ssd` uses the real `WeightPager` from `edgeforge-storage`.
//!
//! ## Feature Flags
//! | Feature | Default | Purpose |
//! |---|---|---|
//! | `quic-transport` | off | Enable real `quinn` + `rustls` (otherwise `LoopbackTransport`) |
//! | `mdns` | off | Enable real `zeroconf` mDNS (otherwise in-memory cache) |
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, NodeId, LayerIdx, ExpertId, Bytes, ModelId
//! - `edgeforge-observability` — FunctionLogger for function-call logging
//! - `edgeforge-storage` — `WeightPager`, `PlatformIo`, `BlockingIo` (for `repage_from_ssd`)
//! - `async-trait`, `tokio` — async runtime
//! - `chrono` — timestamps for heartbeat
//! - `parking_lot` — thread-safe mutexes
//! - `ulid` — NodeId generation in loopback transport
//! - `ed25519-dalek` + `ring` — Ed25519 mTLS (always available)
//! - `base64` + `hex` — encoding
//! - `quinn` + `rustls` + `rcgen` + `rustls-pemfile` — real QUIC+mTLS (feature-gated)
//! - `zeroconf` — real mDNS (feature-gated)
//!
//! ## Output
//! `MeshConn` — established QUIC (or loopback) connection to a peer.
//! `PeerCert` — a peer's Ed25519-signed certificate.
//! `BandwidthMeasurement` — measured bandwidth + RTT.
//!
//! ## Function Logging
//! Every public function logs its entry and exit status via the
//! `FunctionLogger`. Log format:
//! `timestamp - crate - file - function - status`

pub mod bandwidth;
pub mod capability;
pub mod discovery;
pub mod mesh;
pub mod mtls;
pub mod pipeline;
pub mod quic_transport;
pub mod recovery;

pub use bandwidth::{probe_bandwidth, probe_bandwidth_loopback, BandwidthMeasurement, SUFFICIENT_BANDWIDTH_BPS};
pub use mesh::{Mesh, MeshEvent};
pub use mtls::{verify_peer_cert, LocalCa, PeerCert};
pub use pipeline::{forward_activations, forward_activations_loopback, PipelinePlan, PipelineStage};
pub use quic_transport::{
    ActStream, ConnMetadata, Heartbeat, LoopbackTransport, MeshConn, MeshTransport, PeerAddr,
    QuinnTransport, TransportConfig, WeightStream,
};
pub use recovery::{repage_from_ssd, repage_from_ssd_simple, HeartbeatConfig};

/// Global function logger for the edgeforge-net crate.
/// Logs every function call in the format:
/// `timestamp - edgeforge-net - file.rs - function - STATUS`
pub static NET_LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);
