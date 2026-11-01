// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::ControlEvent;
use tokio::sync::broadcast;
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<ControlEvent>,
}
impl EventBus {
    pub fn new(cap: usize) -> Self {
        let (s, _) = broadcast::channel(cap);
        Self { sender: s }
    }
    pub fn publish(&self, e: ControlEvent) {
        let _ = self.sender.send(e);
    }
    pub fn subscribe(&self) -> broadcast::Receiver<ControlEvent> {
        self.sender.subscribe()
    }
}
impl Default for EventBus {
    fn default() -> Self {
        Self::new(4096)
    }
}
