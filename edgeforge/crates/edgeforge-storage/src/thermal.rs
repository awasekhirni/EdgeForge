// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::Celsius;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
#[derive(Clone, Debug)]
pub struct ThermalPolicy {
    pub poll_interval: Duration,
    pub throttle_at: Celsius,
    pub pause_at: Celsius,
}
impl Default for ThermalPolicy {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            throttle_at: Celsius::THROTTLE,
            pause_at: Celsius::CRITICAL,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct ThermalState {
    pub current: Option<Celsius>,
    pub throttled: bool,
    pub paused: bool,
}
pub struct ThermalMonitor {
    policy: ThermalPolicy,
    state: Arc<RwLock<ThermalState>>,
}
impl ThermalMonitor {
    pub fn new(policy: ThermalPolicy) -> Self {
        Self {
            policy,
            state: Arc::new(RwLock::new(ThermalState::default())),
        }
    }
    pub async fn sample(&self, temp: Celsius) -> ThermalState {
        let mut state = self.state.write().await;
        state.current = Some(temp);
        state.throttled = temp.0 >= self.policy.throttle_at.0;
        state.paused = temp.0 >= self.policy.pause_at.0;
        state.clone()
    }
    pub async fn state(&self) -> ThermalState {
        self.state.read().await.clone()
    }
}
