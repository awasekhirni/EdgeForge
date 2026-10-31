// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::LayerIdx;
use std::sync::Arc;
use tokio::sync::Notify;
#[derive(Clone, Debug)]
pub struct PrefetchConfig {
    pub depth: u32,
    pub thermal_throttle: bool,
}
impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            depth: 1,
            thermal_throttle: false,
        }
    }
}
pub struct Prefetcher {
    config: PrefetchConfig,
    notify: Arc<Notify>,
}
impl Prefetcher {
    pub fn new(config: PrefetchConfig) -> Self {
        Self {
            config,
            notify: Arc::new(Notify::new()),
        }
    }
    pub fn notify_layer_done(&self, _layer: LayerIdx) {
        self.notify.notify_waiters();
    }
    pub fn config(&self) -> &PrefetchConfig {
        &self.config
    }
    pub fn set_thermal_throttle(&mut self, on: bool) {
        self.config.thermal_throttle = on;
    }
}
