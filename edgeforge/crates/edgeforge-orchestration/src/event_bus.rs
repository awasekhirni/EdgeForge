// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # ControlEvent Publisher (ADR-019)
//!
//! ## Purpose
//! The single publisher that all subsystems use to emit control events.
//! The streaming graph, MAPE-K loop, and OTLP exporter all subscribe.
//!
//! ## What This File Does
//! - `EventBus` — broadcast channel for ControlEvent
//! - `new(capacity)` — create a new bus with the given channel capacity
//! - `publish(event)` — publish an event to all subscribers
//! - `subscribe()` — get a receiver for the event stream
//! - `subscriber_count()` — number of active subscribers
//!
//! ## Dependencies
//! - `tokio::sync::broadcast` — broadcast channel
//! - `edgeforge_core::ControlEvent` — the event type
//!
//! ## Output
//! `broadcast::Receiver<ControlEvent>` — from subscribe().
//!
//! ## Note
//! Subscribers that fall behind receive `RecvError::Lagged` — they must
//! tolerate gaps (R-18 load shedding).

use edgeforge_core::ControlEvent;
use tokio::sync::broadcast;
#[derive(Clone)]
/// `EventBus` — see module documentation for purpose and usage.
pub struct EventBus {
    sender: broadcast::Sender<ControlEvent>,
}
impl EventBus {
    /// Creates a new instance with default configuration.
    pub fn new(cap: usize) -> Self {
        let (s, _) = broadcast::channel(cap);
        Self { sender: s }
    }
    /// Publishes an event to all subscribers.
    pub fn publish(&self, e: ControlEvent) {
        let _ = self.sender.send(e);
    }
    /// Returns a receiver for the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<ControlEvent> {
        self.sender.subscribe()
    }
    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}
impl Default for EventBus {
    fn default() -> Self {
        Self::new(4096)
    }
}
