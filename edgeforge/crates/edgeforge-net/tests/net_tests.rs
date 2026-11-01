// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Network Tests — edgeforge-net
//!
//! Tests for: QuinnTransport stub, mesh event channel.

#[cfg(test)]
mod tests {
    use edgeforge_net::*;

    #[tokio::test]
    async fn test_quinn_transport_connect_not_implemented() {
        let t = quic_transport::QuinnTransport::new();
        let addr: std::net::SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let peer = quic_transport::PeerAddr(addr);
        let result = t.connect(&peer).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mesh_event_channel() {
        let (mesh, mut rx) = mesh::Mesh::new();
        // Verify the channel was created
        assert_eq!(mesh._events_tx.capacity(), 64);
        // Verify receiver exists
        let _ = rx.try_recv(); // should be empty
    }

    #[test]
    fn test_peer_addr_display() {
        let addr: std::net::SocketAddr = "127.0.0.1:11434".parse().unwrap();
        let peer = quic_transport::PeerAddr(addr);
        assert_eq!(peer.to_string(), "127.0.0.1:11434");
    }
}
