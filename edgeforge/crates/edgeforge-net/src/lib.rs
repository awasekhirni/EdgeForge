// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-net — L2 Networking (ADR-010 QUIC)
//!
//! ## Purpose
//! QUIC-based mesh transport with mTLS, mDNS peer discovery, bandwidth probing,
//! pipeline parallelism, and heartbeat-driven recovery.
//!
//! ## Modules
//! - `quic_transport` — MeshTransport trait + QuinnTransport (QUIC + mTLS)
//! - `mesh` — Mesh coordinator + MeshEvent
//! - `discovery` — mDNS peer discovery (F-NET-01)
//! - `mtls` — Per-node cert generation + rustls verification (F-SEC-02)
//! - `capability` — PeerCapability descriptor (G-03 fix)
//! - `pipeline` — Layer-stripe pipeline parallelism (F-NET-04)
//! - `bandwidth` — Link bandwidth prober (F-NET-05)
//! - `recovery` — Heartbeat-driven peer eviction + expert re-page (F-NET-06)
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, NodeId
//! - `edgeforge-observability` — FunctionLogger for function-call logging
//! - `async-trait`, `tokio` — async runtime
//! - `chrono` — timestamps for heartbeat
//!
//! ## Output
//! `MeshConn` — established QUIC connection to a peer.
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

pub use mesh::{Mesh, MeshEvent};
pub use quic_transport::{MeshConn, MeshTransport, PeerAddr, QuinnTransport};

/// Global function logger for the edgeforge-net crate.
/// Logs every function call in the format:
/// `timestamp - edgeforge-net - file.rs - function - STATUS`
pub static NET_LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);
