//! # Per-Layer Latency Profiler (F-MON-05)
//!
//! ## Purpose
//! Traces per-layer wall time and surfaces the top-N slowest layers in the UI.
//!
//! ## What This File Does
//! - `LayerLatency` — a single latency sample (layer + duration)
//! - `Profiler` — accumulates per-layer latency samples
//! - `record(layer, duration)` — record a sample
//! - `mean_latency(layer)` — mean latency for a layer
//! - `top_slowest(n)` — top-N slowest layers by mean latency
//!
//! ## Dependencies
//! - `edgeforge_core::LayerIdx`
//! - `std::collections::HashMap`
//! - `std::time::Duration`
//!
//! ## Output
//! `Vec<(LayerIdx, Duration)>` from top_slowest() — the N slowest layers.

use edgeforge_core::LayerIdx;
use std::collections::HashMap;
use std::time::Duration;

/// A per-layer latency sample.
#[derive(Clone, Copy, Debug)]
pub struct LayerLatency {
    pub layer: LayerIdx,
    pub duration: Duration,
}

/// The profiler. Accumulates per-layer latency samples.
pub struct Profiler {
    samples: HashMap<LayerIdx, Vec<Duration>>,
}

impl Profiler {
    /// Create a new empty profiler.
    pub fn new() -> Self {
        Self {
            samples: HashMap::new(),
        }
    }

    /// Record a per-layer latency sample.
    ///
    /// ## Parameters
    /// - `layer`: The layer index
    /// - `duration`: The wall-clock duration of the layer's computation
    pub fn record(&mut self, layer: LayerIdx, duration: Duration) {
        self.samples.entry(layer).or_default().push(duration);
    }

    /// Get the mean latency for a specific layer.
    ///
    /// ## Parameters
    /// - `layer`: The layer index
    ///
    /// ## Returns
    /// `Option<Duration>` — the mean, or None if no samples.
    pub fn mean_latency(&self, layer: LayerIdx) -> Option<Duration> {
        let samples = self.samples.get(&layer)?;
        if samples.is_empty() {
            return None;
        }
        let total: u128 = samples.iter().map(|d| d.as_nanos()).sum();
        Some(Duration::from_nanos((total / samples.len() as u128) as u64))
    }

    /// Get the top-N slowest layers by mean latency.
    ///
    /// ## Parameters
    /// - `n`: Number of layers to return
    ///
    /// ## Returns
    /// `Vec<(LayerIdx, Duration)>` — the N slowest layers, sorted descending.
    pub fn top_slowest(&self, n: usize) -> Vec<(LayerIdx, Duration)> {
        let mut means: Vec<(LayerIdx, Duration)> = self
            .samples
            .keys()
            .filter_map(|&layer| self.mean_latency(layer).map(|d| (layer, d)))
            .collect();
        means.sort_by(|a, b| b.1.cmp(&a.1));
        means.into_iter().take(n).collect()
    }

    /// Get the total number of samples recorded.
    pub fn total_samples(&self) -> usize {
        self.samples.values().map(|v| v.len()).sum()
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_mean() {
        let mut p = Profiler::new();
        p.record(LayerIdx(0), Duration::from_millis(10));
        p.record(LayerIdx(0), Duration::from_millis(20));
        assert_eq!(p.mean_latency(LayerIdx(0)), Some(Duration::from_millis(15)));
    }

    #[test]
    fn top_slowest_orders_descending() {
        let mut p = Profiler::new();
        p.record(LayerIdx(0), Duration::from_millis(5));
        p.record(LayerIdx(1), Duration::from_millis(20));
        p.record(LayerIdx(2), Duration::from_millis(10));
        let top = p.top_slowest(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, LayerIdx(1)); // 20ms is slowest
        assert_eq!(top[1].0, LayerIdx(2)); // 10ms is second
    }

    #[test]
    fn mean_returns_none_for_missing_layer() {
        let p = Profiler::new();
        assert!(p.mean_latency(LayerIdx(99)).is_none());
    }

    #[test]
    fn total_samples() {
        let mut p = Profiler::new();
        p.record(LayerIdx(0), Duration::from_millis(1));
        p.record(LayerIdx(0), Duration::from_millis(2));
        p.record(LayerIdx(1), Duration::from_millis(3));
        assert_eq!(p.total_samples(), 3);
    }
}
