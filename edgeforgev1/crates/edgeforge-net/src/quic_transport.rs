// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use async_trait::async_trait;
use edgeforge_core::{EdgeForgeError, NodeId};
use std::net::SocketAddr;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerAddr(pub SocketAddr);
impl std::fmt::Display for PeerAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
pub struct MeshConn {
    pub peer: PeerAddr,
    pub peer_id: NodeId,
}
impl MeshConn {
    pub fn peer(&self) -> PeerAddr {
        self.peer
    }
    pub fn peer_id(&self) -> NodeId {
        self.peer_id
    }
}
pub struct WeightStream;
pub struct ActStream;
pub struct Heartbeat;
#[async_trait]
pub trait MeshTransport: Send + Sync + 'static {
    async fn connect(&self, peer: &PeerAddr) -> Result<MeshConn, EdgeForgeError>;
    async fn open_weight_stream(&self, conn: &MeshConn) -> Result<WeightStream, EdgeForgeError>;
    async fn open_activation_stream(&self, conn: &MeshConn) -> Result<ActStream, EdgeForgeError>;
    async fn heartbeat(&self, conn: &MeshConn) -> Result<Heartbeat, EdgeForgeError>;
}
pub struct QuinnTransport;
impl QuinnTransport {
    pub fn new() -> Self {
        Self
    }
}
impl Default for QuinnTransport {
    fn default() -> Self {
        Self::new()
    }
}
#[async_trait]
impl MeshTransport for QuinnTransport {
    async fn connect(&self, _peer: &PeerAddr) -> Result<MeshConn, EdgeForgeError> {
        Err(EdgeForgeError::NotImplemented(
            "QuinnTransport::connect (Phase 3)",
        ))
    }
    async fn open_weight_stream(&self, _conn: &MeshConn) -> Result<WeightStream, EdgeForgeError> {
        Err(EdgeForgeError::NotImplemented("Phase 3"))
    }
    async fn open_activation_stream(&self, _conn: &MeshConn) -> Result<ActStream, EdgeForgeError> {
        Err(EdgeForgeError::NotImplemented("Phase 3"))
    }
    async fn heartbeat(&self, _conn: &MeshConn) -> Result<Heartbeat, EdgeForgeError> {
        Err(EdgeForgeError::NotImplemented("Phase 3"))
    }
}
