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
pub struct EventBus { sender: broadcast::Sender<ControlEvent> }
impl EventBus {
    /// Creates a new instance with default configuration.
    pub fn new(cap: usize) -> Self { let (s, _) = broadcast::channel(cap); Self { sender: s } }
    /// Publishes an event to all subscribers.
    pub fn publish(&self, e: ControlEvent) { let _ = self.sender.send(e); }
    /// Returns a receiver for the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<ControlEvent> { self.sender.subscribe() }
    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize { self.sender.receiver_count() }
}
impl Default for EventBus { fn default() -> Self { Self::new(4096) } }

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::{ControlEvent, ControlEventKind, NodeId};
    use ulid::Ulid;

    fn make_event() -> ControlEvent {
        ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::PeerLost { peer: NodeId(Ulid::new()) },
        }
    }

    #[test]
    fn test_new_event_bus_has_no_subscribers() {
        let bus = EventBus::new(16);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_subscribe_increases_count() {
        let bus = EventBus::new(16);
        let _rx = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_default_capacity() {
        let bus = EventBus::default();
        let _rx = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[tokio::test]
    async fn test_publish_delivers_to_subscriber() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        bus.publish(make_event());
        let received = rx.recv().await;
        assert!(received.is_ok());
    }

    #[tokio::test]
    async fn test_publish_delivers_to_multiple_subscribers() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        bus.publish(make_event());
        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn test_publish_without_subscribers_is_ok() {
        let bus = EventBus::new(16);
        bus.publish(make_event());
        // Should not panic
    }
}
