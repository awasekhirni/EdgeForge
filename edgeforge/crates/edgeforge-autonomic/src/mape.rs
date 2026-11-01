// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::{
    AutonomicAction, AutonomicKnowledge, FailurePattern, GeneratedTest, Remediation,
    RemediationResult, TestGenerator,
};
use edgeforge_core::ControlEvent;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
#[derive(Clone, Debug)]
pub struct MapeKConfig {
    pub analyze_interval: Duration,
    pub analyze_batch_size: usize,
    pub control_limit_sigmas: f64,
    pub commit_generated_tests: bool,
}
impl Default for MapeKConfig {
    fn default() -> Self {
        Self {
            analyze_interval: Duration::from_secs(10),
            analyze_batch_size: 256,
            control_limit_sigmas: 3.0,
            commit_generated_tests: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariationKind {
    CommonCause,
    SpecialCause,
}
pub struct MapeKLoop {
    config: MapeKConfig,
    events_rx: Mutex<mpsc::Receiver<ControlEvent>>,
    knowledge: Arc<AutonomicKnowledge>,
    test_gen: Arc<TestGenerator>,
    stats: Mutex<std::collections::HashMap<String, RollingStats>>,
}
#[derive(Clone, Debug, Default)]
struct RollingStats {
    count: u64,
    mean: f64,
    m2: f64,
}
impl RollingStats {
    fn update(&mut self, v: f64) {
        self.count += 1;
        let d = v - self.mean;
        self.mean += d / self.count as f64;
        self.m2 += d * (v - self.mean);
    }
    fn stddev(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            (self.m2 / (self.count - 1) as f64).sqrt()
        }
    }
    fn classify(&self, v: f64, lim: f64) -> VariationKind {
        if self.count < 30 {
            return VariationKind::SpecialCause;
        }
        let sd = self.stddev();
        if sd == 0.0 {
            return if (v - self.mean).abs() > f64::EPSILON {
                VariationKind::SpecialCause
            } else {
                VariationKind::CommonCause
            };
        }
        if ((v - self.mean).abs() / sd) > lim {
            VariationKind::SpecialCause
        } else {
            VariationKind::CommonCause
        }
    }
}
impl MapeKLoop {
    pub fn new(
        config: MapeKConfig,
        events_rx: mpsc::Receiver<ControlEvent>,
        knowledge: Arc<AutonomicKnowledge>,
        test_gen: Arc<TestGenerator>,
    ) -> Self {
        Self {
            config,
            events_rx: Mutex::new(events_rx),
            knowledge,
            test_gen,
            stats: Mutex::new(std::collections::HashMap::new()),
        }
    }
    pub async fn cycle_once(
        &self,
    ) -> Option<(Remediation, RemediationResult, Option<GeneratedTest>)> {
        let mut batch = Vec::new();
        let mut rx = self.events_rx.lock();
        for _ in 0..self.config.analyze_batch_size {
            match rx.try_recv() {
                Ok(e) => batch.push(e),
                Err(_) => break,
            }
        }
        drop(rx);
        if batch.is_empty() {
            return None;
        }
        let mut classifications = Vec::new();
        for event in &batch {
            if let Some((m, v)) = extract_metric(event) {
                let mut s = self.stats.lock();
                let e = s.entry(m.clone()).or_default();
                let k = e.classify(v, self.config.control_limit_sigmas);
                e.update(v);
                classifications.push((m, v, k));
            }
        }
        for event in &batch {
            self.knowledge.record_event(event.clone());
        }
        let patterns = self.knowledge.detect_patterns(&classifications);
        let remediation = self.plan(&patterns)?;
        let kind = remediation.kind();
        let target = remediation.target_description();
        let result = remediation.execute().await;
        self.knowledge.record_action(AutonomicAction {
            remediation: format!("{kind:?}"),
            target,
            result: result.clone(),
            timestamp_ms: 0,
        });
        let gen = if result.is_success() {
            self.test_gen
                .generate(&patterns, &Remediation::reduce_prefetch_depth())
        } else {
            None
        };
        Some((Remediation::reduce_prefetch_depth(), result, gen))
    }
    fn plan(&self, patterns: &[FailurePattern]) -> Option<Remediation> {
        patterns
            .iter()
            .max_by_key(|p| p.severity())
            .map(|_| Remediation::reduce_prefetch_depth())
    }
}
fn extract_metric(event: &ControlEvent) -> Option<(String, f64)> {
    use edgeforge_core::ControlEventKind::*;
    match &event.kind {
        IoCompletion { latency_us, .. } => Some(("io_latency_us".into(), *latency_us as f64)),
        PeerHeartbeat { rtt_ms, .. } => Some(("peer_rtt_ms".into(), *rtt_ms as f64)),
        ThermalSample { temp_celsius, .. } => Some(("ssd_temp_c".into(), *temp_celsius as f64)),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classify_outlier() {
        let mut s = RollingStats::default();
        for _ in 0..100 {
            s.update(50.0);
        }
        assert_eq!(s.classify(1000.0, 3.0), VariationKind::SpecialCause);
    }
    #[test]
    fn classify_normal() {
        let mut s = RollingStats::default();
        for i in 0..100 {
            s.update(50.0 + (i as f64 % 10.0));
        }
        assert_eq!(s.classify(55.0, 3.0), VariationKind::CommonCause);
    }
}
