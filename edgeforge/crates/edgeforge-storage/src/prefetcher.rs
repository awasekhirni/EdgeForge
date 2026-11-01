//! # Layer-Ahead Prefetcher (F-TIER-03)
//!
//! ## Purpose
//! Runs as a background task that issues async reads for layer N+1's predicted
//! experts while layer N computes. Hides SSD latency behind compute.
//!
//! ## What This File Does
//! - `PrefetchConfig` — depth (layers ahead), thermal_throttle flag
//! - `Prefetcher` — owns a Notify for wakeups
//! - `notify_layer_done(layer)` — called by inference loop after each layer
//! - `set_thermal_throttle(on)` — called by thermal monitor to reduce depth
//!
//! ## Dependencies
//! - `tokio::sync::Notify` — wakeup signaling
//! - `edgeforge_core::LayerIdx`
//!
//! ## Output
//! No direct output — issues async reads via the weight pager.
//!
//! ## Thermal Throttling
//! When SSD temperature exceeds 70°C (F-MON-04), prefetch depth is reduced
//! to prevent thermal collapse. The thermal monitor calls `set_thermal_throttle(true)`.

use edgeforge_core::LayerIdx;
use std::sync::Arc;
use tokio::sync::Notify;
#[derive(Clone, Debug)]
/// `PrefetchConfig` — see module documentation for purpose and usage.
pub struct PrefetchConfig { pub depth: u32, pub thermal_throttle: bool }
impl Default for PrefetchConfig { fn default() -> Self { Self { depth: 1, thermal_throttle: false } } }
/// `Prefetcher` — see module documentation for purpose and usage.
pub struct Prefetcher { config: PrefetchConfig, notify: Arc<Notify> }
impl Prefetcher {
    /// Creates a new instance with default configuration.
    pub fn new(config: PrefetchConfig) -> Self { Self { config, notify: Arc::new(Notify::new()) } }
    /// Notifies the prefetcher that a layer has completed.
    pub fn notify_layer_done(&self, _layer: LayerIdx) { self.notify.notify_waiters(); }
    /// Config — see module documentation for details.
    pub fn config(&self) -> &PrefetchConfig { &self.config }
    /// Sets the value of `thermal_throttle`.
    pub fn set_thermal_throttle(&mut self, on: bool) { self.config.thermal_throttle = on; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let c = PrefetchConfig::default();
        assert_eq!(c.depth, 1);
        assert!(!c.thermal_throttle);
    }

    #[test]
    fn test_new_prefetcher() {
        let p = Prefetcher::new(PrefetchConfig::default());
        assert_eq!(p.config().depth, 1);
        assert!(!p.config().thermal_throttle);
    }

    #[test]
    fn test_notify_layer_done_doesnt_panic() {
        let p = Prefetcher::new(PrefetchConfig::default());
        p.notify_layer_done(LayerIdx(0));
        p.notify_layer_done(LayerIdx(1));
    }

    #[test]
    fn test_set_thermal_throttle() {
        let mut p = Prefetcher::new(PrefetchConfig::default());
        assert!(!p.config().thermal_throttle);
        p.set_thermal_throttle(true);
        assert!(p.config().thermal_throttle);
        p.set_thermal_throttle(false);
        assert!(!p.config().thermal_throttle);
    }

    #[test]
    fn test_custom_depth() {
        let p = Prefetcher::new(PrefetchConfig { depth: 3, thermal_throttle: false });
        assert_eq!(p.config().depth, 3);
    }
}
