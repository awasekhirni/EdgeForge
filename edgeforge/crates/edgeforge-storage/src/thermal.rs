//! # SSD Thermal Monitoring (F-MON-04)
//!
//! ## Purpose
//! Polls SSD temperature and engages throttle/pause when thresholds are exceeded.
//! Prevents SSD thermal collapse during sustained inference.
//!
//! ## What This File Does
//! - `ThermalPolicy` — poll_interval, throttle_at (70°C), pause_at (80°C)
//! - `ThermalState` — current temp, throttled flag, paused flag
//! - `ThermalMonitor` — owns shared state, provides sample() and state()
//! - `sample(temp)` — update state with a new temperature reading
//!
//! ## Dependencies
//! - `tokio::sync::RwLock` — shared thermal state
//! - `edgeforge_core::Celsius` — temperature type with THROTTLE/CRITICAL constants
//! - `std::time::Duration` — poll interval
//!
//! ## Output
//! `ThermalState` — current temperature, throttled flag, paused flag.
//!
//! ## Function: sample
//! - **Purpose:** Push a new temperature sample and update throttle/pause flags
//! - **Parameters:** `Celsius` — the new temperature reading
//! - **Returns:** `ThermalState` — the resulting state
//! - **Side effects:** Sets `throttled = true` if temp >= 70°C; `paused = true` if >= 80°C
//! - **Dependencies:** `self.policy.throttle_at`, `self.policy.pause_at`

use edgeforge_core::Celsius;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
#[derive(Clone, Debug)]
/// `ThermalPolicy` — see module documentation for purpose and usage.
pub struct ThermalPolicy { pub poll_interval: Duration, pub throttle_at: Celsius, pub pause_at: Celsius }
impl Default for ThermalPolicy { fn default() -> Self { Self { poll_interval: Duration::from_secs(5), throttle_at: Celsius::THROTTLE, pause_at: Celsius::CRITICAL } } }
#[derive(Clone, Debug, Default)]
/// `ThermalState` — see module documentation for purpose and usage.
pub struct ThermalState { pub current: Option<Celsius>, pub throttled: bool, pub paused: bool }
/// `ThermalMonitor` — see module documentation for purpose and usage.
pub struct ThermalMonitor { policy: ThermalPolicy, state: Arc<RwLock<ThermalState>> }
impl ThermalMonitor {
    /// Creates a new instance with default configuration.
    pub fn new(policy: ThermalPolicy) -> Self { Self { policy, state: Arc::new(RwLock::new(ThermalState::default())) } }
    /// Takes a temperature sample and updates the thermal state.
    pub async fn sample(&self, temp: Celsius) -> ThermalState {
        let mut state = self.state.write().await;
        state.current = Some(temp);
        state.throttled = temp.0 >= self.policy.throttle_at.0;
        state.paused = temp.0 >= self.policy.pause_at.0;
        state.clone()
    }
    /// Returns the current state.
    pub async fn state(&self) -> ThermalState { self.state.read().await.clone() }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_thermal_policy() {
        let p = ThermalPolicy::default();
        assert_eq!(p.poll_interval, std::time::Duration::from_secs(5));
        assert_eq!(p.throttle_at, Celsius(70.0));
        assert_eq!(p.pause_at, Celsius(80.0));
    }

    #[test]
    fn test_default_thermal_state() {
        let s = ThermalState::default();
        assert!(s.current.is_none());
        assert!(!s.throttled);
        assert!(!s.paused);
    }

    #[tokio::test]
    async fn test_sample_below_throttle() {
        let mon = ThermalMonitor::new(ThermalPolicy::default());
        let state = mon.sample(Celsius(50.0)).await;
        assert!(!state.throttled);
        assert!(!state.paused);
        assert_eq!(state.current, Some(Celsius(50.0)));
    }

    #[tokio::test]
    async fn test_sample_at_throttle_threshold() {
        let mon = ThermalMonitor::new(ThermalPolicy::default());
        let state = mon.sample(Celsius(70.0)).await;
        assert!(state.throttled);
        assert!(!state.paused);
    }

    #[tokio::test]
    async fn test_sample_at_pause_threshold() {
        let mon = ThermalMonitor::new(ThermalPolicy::default());
        let state = mon.sample(Celsius(80.0)).await;
        assert!(state.throttled);
        assert!(state.paused);
    }

    #[tokio::test]
    async fn test_sample_above_pause() {
        let mon = ThermalMonitor::new(ThermalPolicy::default());
        let state = mon.sample(Celsius(85.0)).await;
        assert!(state.throttled);
        assert!(state.paused);
    }

    #[tokio::test]
    async fn test_state_returns_latest() {
        let mon = ThermalMonitor::new(ThermalPolicy::default());
        mon.sample(Celsius(45.0)).await;
        let state = mon.state().await;
        assert_eq!(state.current, Some(Celsius(45.0)));
    }

    #[tokio::test]
    async fn test_state_default_before_sample() {
        let mon = ThermalMonitor::new(ThermalPolicy::default());
        let state = mon.state().await;
        assert!(state.current.is_none());
        assert!(!state.throttled);
    }
}
