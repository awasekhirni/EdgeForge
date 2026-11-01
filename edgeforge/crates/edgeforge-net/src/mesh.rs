//! # Mesh Coordinator
//!
//! ## Purpose
//! Discovers peers, maintains the connection pool, routes expert fetch requests
//! to the fastest source.
//!
//! ## What This File Does
//! - `MeshEvent` — PeerDiscovered, PeerLost, HeartbeatTick
//! - `Mesh` — holds an mpsc sender for events
//! - `Mesh::new()` — creates a Mesh + event receiver pair
//!
//! ## Dependencies
//! - `tokio::sync::mpsc` — event channel
//! - `edgeforge_core::NodeId`
//! - `edgeforge_observability` — FunctionGuard for call logging

use edgeforge_core::NodeId;
use edgeforge_observability::function_logger::FunctionGuard;
use tokio::sync::mpsc;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> = std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// Events emitted by the mesh coordinator.
#[derive(Clone, Debug)]
pub enum MeshEvent {
    /// A new peer was discovered via mDNS (F-NET-01).
    PeerDiscovered { id: NodeId, hostname: String },
    /// A peer disconnected; its experts are now unreachable.
    PeerLost { id: NodeId },
    /// A heartbeat round completed.
    HeartbeatTick,
}

/// Mesh coordinator handle.
///
/// ## Purpose
/// Owns the sender side of the mesh event channel. The receiver is returned
/// to the caller for event processing.
pub struct Mesh {
    /// The event channel sender.
    pub _events_tx: mpsc::Sender<MeshEvent>,
}

impl Mesh {
    /// Creates a new mesh coordinator and returns the event receiver.
    ///
    /// ## Purpose
    /// Initializes the mesh event channel. The caller receives the receiver
    /// end to process mesh events (peer discovered, peer lost, heartbeat tick).
    ///
    /// ## Parameters
    /// - `capacity`: Channel capacity (default 64).
    ///
    /// ## Returns
    /// `(Mesh, Receiver<MeshEvent>)` — the coordinator + event stream.
    pub fn new() -> (Self, mpsc::Receiver<MeshEvent>) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "mesh.rs", "Mesh::new");
        let (tx, rx) = mpsc::channel(64);
        (Self { _events_tx: tx }, rx)
    }

    /// Creates a mesh with a custom channel capacity.
    pub fn with_capacity(capacity: usize) -> (Self, mpsc::Receiver<MeshEvent>) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "mesh.rs", "with_capacity");
        let (tx, rx) = mpsc::channel(capacity);
        (Self { _events_tx: tx }, rx)
    }
}

impl Default for Mesh {
    fn default() -> Self {
        let (s, _) = Self::new();
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_channel_with_capacity() {
        let (mesh, mut rx) = Mesh::new();
        assert_eq!(mesh._events_tx.capacity(), 64);
        // Receiver should be empty
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_with_capacity_custom() {
        let (mesh, mut rx) = Mesh::with_capacity(128);
        assert_eq!(mesh._events_tx.capacity(), 128);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_send_and_receive_event() {
        let (mesh, mut rx) = Mesh::new();
        let event = MeshEvent::HeartbeatTick;
        let _ = mesh._events_tx.send(event).await;
        let received = rx.recv().await;
        assert!(matches!(received, Some(MeshEvent::HeartbeatTick)));
    }

    #[test]
    fn test_mesh_event_variants() {
        let peer = edgeforge_core::NodeId::new();
        let _ = MeshEvent::PeerDiscovered { id: peer, hostname: "test".into() };
        let _ = MeshEvent::PeerLost { id: peer };
        let _ = MeshEvent::HeartbeatTick;
    }

    #[test]
    fn test_default_creates_mesh() {
        let (mesh, _rx) = Mesh::default();
        assert_eq!(mesh._events_tx.capacity(), 64);
    }
}
