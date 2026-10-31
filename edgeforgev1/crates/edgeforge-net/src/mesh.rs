// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::NodeId;
use tokio::sync::mpsc;
#[derive(Clone, Debug)]
pub enum MeshEvent {
    PeerDiscovered { id: NodeId, hostname: String },
    PeerLost { id: NodeId },
    HeartbeatTick,
}
pub struct Mesh {
    pub _events_tx: mpsc::Sender<MeshEvent>,
}
impl Mesh {
    pub fn new() -> (Self, mpsc::Receiver<MeshEvent>) {
        let (tx, rx) = mpsc::channel(64);
        (Self { _events_tx: tx }, rx)
    }
}
impl Default for Mesh {
    fn default() -> Self {
        let (s, _) = Self::new();
        s
    }
}
