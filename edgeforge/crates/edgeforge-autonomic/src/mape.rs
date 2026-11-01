//! # MAPE-K Loop Implementation (ADR-029)
//!
//! ## Purpose
//! Runs the Monitor → Analyze → Plan → Execute cycle. Uses Statistical Process
//! Control (Welford's algorithm) to distinguish common-cause variation
//! (systemic) from special-cause variation (one-off).
//!
//! ## What This File Does
//! - `MapeKConfig` — analyze_interval, batch_size, control_limit_sigmas, commit_generated_tests
//! - `VariationKind` — CommonCause (systemic) / SpecialCause (one-off)
//! - `MapeKLoop` — the loop orchestrator
//! - `RollingStats` — Welford's online mean/variance calculator
//! - `cycle_once()` — run one MAPE cycle, returns (remediation, result, generated_test)
//!
//! ## The MAPE Cycle
//! 1. **Monitor** — drain a batch of ControlEvents from the event channel
//! 2. **Analyze** — classify each event's metric via SPC (Welford's algorithm)
//! 3. **Plan** — select the highest-severity failure pattern + choose a remediation
//! 4. **Execute** — apply the remediation against the `RemediationContext`
//! 5. **Knowledge** — record the action; if the remediation succeeded, generate
//!    a regression test (antifragile property)
//!
//! ## Dependencies
//! - `crate::{AutonomicKnowledge, FailurePattern, Remediation, RemediationContext, TestGenerator}`
//! - `edgeforge_core::ControlEvent` — event stream
//! - `parking_lot::Mutex` — for rolling stats
//! - `tokio::sync::mpsc` — event receiver
//!
//! ## Output
//! `Option<(Remediation, RemediationResult, Option<GeneratedTest>)>` — the
//! remediation applied, its result, and any generated regression test.
//!
//! ## SPC Classification
//! - **Common-cause** — within 3σ of rolling mean → systemic, requires process change
//! - **Special-cause** — beyond 3σ → one-off, requires targeted fix
//! - Requires ≥30 samples before classification is meaningful

use crate::{
    AutonomicAction, AutonomicKnowledge, FailureKind, FailurePattern, GeneratedTest,
    Remediation, RemediationContext, RemediationKind, RemediationResult, TestGenerator,
};
use edgeforge_core::ControlEvent;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Configuration for the MAPE-K loop.
#[derive(Clone, Debug)]
pub struct MapeKConfig {
    /// How often to run the Analyze phase.
    pub analyze_interval: Duration,
    /// Maximum number of events to process per cycle.
    pub analyze_batch_size: usize,
    /// Number of standard deviations for the SPC control limit (default: 3.0).
    pub control_limit_sigmas: f64,
    /// Whether to write generated tests to disk (default: false — tests are
    /// returned but not committed to the repo).
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

/// The kind of variation detected by SPC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariationKind {
    /// Common-cause variation — within 3σ of the rolling mean. Systemic.
    CommonCause,
    /// Special-cause variation — beyond 3σ. One-off, requires targeted fix.
    SpecialCause,
}

/// The MAPE-K loop orchestrator.
pub struct MapeKLoop {
    config: MapeKConfig,
    events_rx: Mutex<mpsc::Receiver<ControlEvent>>,
    knowledge: Arc<AutonomicKnowledge>,
    test_gen: Arc<TestGenerator>,
    remediation_ctx: Arc<RemediationContext>,
    stats: Mutex<std::collections::HashMap<String, RollingStats>>,
}

impl MapeKLoop {
    /// Creates a new MAPE-K loop.
    ///
    /// ## Parameters
    /// - `config`: Loop configuration.
    /// - `events_rx`: Receiver for the ControlEvent stream.
    /// - `knowledge`: Shared knowledge base.
    /// - `test_gen`: Test generator (for antifragile test generation).
    /// - `remediation_ctx`: Remediation context (owns the mutable state).
    #[must_use]
    pub fn new(
        config: MapeKConfig,
        events_rx: mpsc::Receiver<ControlEvent>,
        knowledge: Arc<AutonomicKnowledge>,
        test_gen: Arc<TestGenerator>,
        remediation_ctx: Arc<RemediationContext>,
    ) -> Self {
        Self {
            config,
            events_rx: Mutex::new(events_rx),
            knowledge,
            test_gen,
            remediation_ctx,
            stats: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Runs one MAPE-K cycle.
    ///
    /// ## Returns
    /// `Option<(Remediation, RemediationResult, Option<GeneratedTest>)>` —
    /// the remediation applied, its result, and any generated regression test.
    /// Returns `None` if no events were processed or no patterns were detected.
    pub async fn cycle_once(
        &self,
    ) -> Option<(Remediation, RemediationResult, Option<GeneratedTest>)> {
        // ── Monitor: drain a batch of events ──
        let mut batch = Vec::new();
        {
            let mut rx = self.events_rx.lock();
            for _ in 0..self.config.analyze_batch_size {
                match rx.try_recv() {
                    Ok(e) => batch.push(e),
                    Err(_) => break,
                }
            }
        }
        if batch.is_empty() {
            return None;
        }

        // ── Analyze: classify each event's metric via SPC ──
        let mut classifications = Vec::new();
        {
            let mut stats = self.stats.lock();
            for event in &batch {
                if let Some((metric, value)) = extract_metric(event) {
                    let entry = stats.entry(metric.clone()).or_default();
                    let kind = entry.classify(value, self.config.control_limit_sigmas);
                    entry.update(value);
                    classifications.push((metric, value, kind));
                }
            }
        }

        // Record events in the knowledge base.
        for event in &batch {
            self.knowledge.record_event(event.clone());
        }

        // ── Knowledge: detect failure patterns ──
        let patterns = self.knowledge.detect_patterns(&classifications);
        if patterns.is_empty() {
            return None;
        }

        // ── Plan: select remediation based on the highest-severity pattern ──
        let remediation = self.plan(&patterns)?;

        // ── Execute: apply the remediation against the context ──
        let kind = remediation.kind();
        let target = remediation.target_description();
        let result = remediation.execute(&self.remediation_ctx).await;

        // Record the action in the knowledge base.
        self.knowledge.record_action(AutonomicAction {
            remediation: format!("{kind}"),
            target,
            result: result.clone(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        });

        // ── Knowledge: antifragile test generation ──
        let generated_test = if result.is_success() {
            self.test_gen.generate(&patterns, &remediation)
        } else {
            None
        };

        Some((remediation, result, generated_test))
    }

    /// Plans a remediation based on the detected failure patterns.
    ///
    /// ## Algorithm
    /// 1. Find the highest-severity pattern.
    /// 2. Map the failure kind to a remediation:
    ///    - `ThermalThrottle` → `ReducePrefetchDepth`
    ///    - `CorruptExpert` → `EvictExpert` (with the pattern's target)
    ///    - `PeerDisconnect` → `EvictPeer` (with the pattern's target)
    ///    - `SpeculationMissSpike` → `ReduceDraftMax`
    ///    - `VerificationFailure` → `TriggerKaniReverification` (with the pattern's target)
    fn plan(&self, patterns: &[FailurePattern]) -> Option<Remediation> {
        let pattern = patterns.iter().max_by_key(|p| p.severity())?;

        let remediation = match pattern.kind {
            FailureKind::ThermalThrottle => Remediation::reduce_prefetch_depth(),
            FailureKind::CorruptExpert => {
                let target = pattern
                    .target
                    .clone()
                    .unwrap_or_else(|| "layer_0_expert_0".to_string());
                Remediation::evict_expert(target)
            }
            FailureKind::PeerDisconnect => {
                let target = pattern
                    .target
                    .clone()
                    .unwrap_or_else(|| "peer-unknown".to_string());
                Remediation::evict_peer(target)
            }
            FailureKind::SpeculationMissSpike => Remediation::reduce_draft_max(),
            FailureKind::VerificationFailure => {
                let target = pattern
                    .target
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                Remediation::trigger_kani_reverification(target)
            }
        };

        tracing::info!(
            pattern_kind = %pattern.kind,
            pattern_severity = pattern.severity(),
            remediation_kind = ?remediation.kind(),
            "MAPE-K Plan: selected remediation"
        );

        Some(remediation)
    }

    /// Returns a reference to the remediation context.
    #[must_use]
    pub fn remediation_context(&self) -> &Arc<RemediationContext> {
        &self.remediation_ctx
    }

    /// Returns a reference to the knowledge base.
    #[must_use]
    pub fn knowledge(&self) -> &Arc<AutonomicKnowledge> {
        &self.knowledge
    }
}

/// Rolling statistics for Welford's online algorithm.
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

    fn classify(&self, v: f64, limit: f64) -> VariationKind {
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
        if ((v - self.mean).abs() / sd) > limit {
            VariationKind::SpecialCause
        } else {
            VariationKind::CommonCause
        }
    }
}

/// Extract a (metric_name, value) pair from a ControlEvent for SPC.
fn extract_metric(event: &ControlEvent) -> Option<(String, f64)> {
    use edgeforge_core::ControlEventKind::*;
    match &event.kind {
        IoCompletion { latency_us, .. } => Some(("io_latency_us".into(), *latency_us as f64)),
        PeerHeartbeat { rtt_ms, .. } => Some(("peer_rtt_ms".into(), *rtt_ms as f64)),
        ThermalSample { temp_celsius, .. } => Some(("ssd_temp_c".into(), *temp_celsius as f64)),
        _ => None,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::{
        ControlEvent, ControlEventKind, CrowdTier, ExpertId, ExpertSource, JobId, LayerIdx,
        ModelId, NodeId, SpeculationHorizon, VerificationOutcome,
    };
    use ulid::Ulid;

    fn make_thermal_event(temp: f32, throttled: bool) -> ControlEvent {
        ControlEvent {
            timestamp_ms: 1700000000000,
            kind: ControlEventKind::ThermalSample {
                temp_celsius: temp,
                throttled,
                paused: false,
            },
        }
    }

    fn make_peer_lost_event(peer: NodeId) -> ControlEvent {
        ControlEvent {
            timestamp_ms: 1700000000000,
            kind: ControlEventKind::PeerLost { peer },
        }
    }

    fn make_speculation_miss_event() -> ControlEvent {
        ControlEvent {
            timestamp_ms: 1700000000000,
            kind: ControlEventKind::SpeculationVerdict {
                horizon: SpeculationHorizon::H1,
                hit: false,
                surrogate_used: false,
                accuracy_delta: None,
            },
        }
    }

    fn make_verification_failed_event(peer: NodeId) -> ControlEvent {
        ControlEvent {
            timestamp_ms: 1700000000000,
            kind: ControlEventKind::VerificationVerdict {
                job: JobId(Ulid::new()),
                peer,
                tier: CrowdTier::Trusted,
                outcome: VerificationOutcome::Failed,
                latency_ms: 50,
            },
        }
    }

    fn make_loop(events: Vec<ControlEvent>) -> (MapeKLoop, Arc<RemediationContext>) {
        let (tx, rx) = mpsc::channel(256);
        for event in events {
            let _ = tx.try_send(event);
        }
        let knowledge = Arc::new(AutonomicKnowledge::new());
        let test_gen = TestGenerator::new();
        let ctx = Arc::new(RemediationContext::new());
        let mape = MapeKLoop::new(
            MapeKConfig::default(),
            rx,
            knowledge,
            test_gen,
            ctx.clone(),
        );
        (mape, ctx)
    }

    // ── RollingStats tests ──

    #[test]
    fn test_classify_outlier() {
        let mut s = RollingStats::default();
        for _ in 0..100 {
            s.update(50.0);
        }
        assert_eq!(s.classify(1000.0, 3.0), VariationKind::SpecialCause);
    }

    #[test]
    fn test_classify_normal() {
        let mut s = RollingStats::default();
        for i in 0..100 {
            s.update(50.0 + (i as f64 % 10.0));
        }
        assert_eq!(s.classify(55.0, 3.0), VariationKind::CommonCause);
    }

    #[test]
    fn test_classify_few_samples_is_special_cause() {
        let mut s = RollingStats::default();
        s.update(50.0);
        s.update(51.0);
        // < 30 samples → always SpecialCause
        assert_eq!(s.classify(50.5, 3.0), VariationKind::SpecialCause);
    }

    #[test]
    fn test_rolling_stats_stddev() {
        let mut s = RollingStats::default();
        for v in &[1.0, 2.0, 3.0, 4.0, 5.0] {
            s.update(*v);
        }
        // Mean = 3.0, sample variance = 2.5, stddev ≈ 1.58
        let sd = s.stddev();
        assert!((sd - 1.5811).abs() < 0.01, "stddev should be ~1.58, got {sd}");
    }

    // ── extract_metric tests ──

    #[test]
    fn test_extract_metric_thermal() {
        let event = make_thermal_event(50.0, false);
        let (metric, value) = extract_metric(&event).unwrap();
        assert_eq!(metric, "ssd_temp_c");
        assert!((value - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_metric_io_completion() {
        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::IoCompletion {
                sequence: 0,
                layer: LayerIdx(0),
                expert: ExpertId(0),
                source: ExpertSource::LocalSsd,
                bytes: 1024,
                latency_us: 500,
            },
        };
        let (metric, value) = extract_metric(&event).unwrap();
        assert_eq!(metric, "io_latency_us");
        assert!((value - 500.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_metric_peer_heartbeat() {
        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::PeerHeartbeat {
                peer: NodeId(Ulid::new()),
                rtt_ms: 25,
                thermal_celsius: 45.0,
                experts_resident: 10,
            },
        };
        let (metric, value) = extract_metric(&event).unwrap();
        assert_eq!(metric, "peer_rtt_ms");
        assert!((value - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_metric_no_metric_for_router_decision() {
        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::RouterDecision {
                sequence: 0,
                layer: LayerIdx(0),
                experts: vec![ExpertId(0)],
            },
        };
        assert!(extract_metric(&event).is_none());
    }

    // ── cycle_once tests — the full MAPE-K cycle ──

    #[tokio::test]
    async fn test_cycle_once_no_events_returns_none() {
        let (mape, _ctx) = make_loop(vec![]);
        let result = mape.cycle_once().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cycle_once_thermal_throttle() {
        let (mape, ctx) = make_loop(vec![
            make_thermal_event(75.0, true),
        ]);

        let result = mape.cycle_once().await;
        assert!(result.is_some(), "should produce a remediation");

        let (remediation, rem_result, gen_test) = result.unwrap();
        assert_eq!(remediation.kind(), RemediationKind::ReducePrefetchDepth);
        assert!(rem_result.is_success(), "remediation should succeed");
        // The context should have been mutated.
        assert_eq!(ctx.prefetch_depth(), 3, "prefetch depth should be reduced");
        // A regression test should have been generated (antifragile).
        assert!(gen_test.is_some(), "should generate a regression test");
        let test = gen_test.unwrap();
        assert!(test.name.contains("thermal_throttle"));
    }

    #[tokio::test]
    async fn test_cycle_once_peer_disconnect() {
        let peer = NodeId(Ulid::new());
        let (mape, ctx) = make_loop(vec![make_peer_lost_event(peer)]);

        let result = mape.cycle_once().await;
        assert!(result.is_some());

        let (remediation, rem_result, _gen) = result.unwrap();
        assert_eq!(remediation.kind(), RemediationKind::EvictPeer);
        assert!(rem_result.is_success());
        assert!(ctx.is_peer_evicted(&peer.to_string()));
    }

    #[tokio::test]
    async fn test_cycle_once_speculation_miss_spike() {
        let (mape, ctx) = make_loop(vec![
            make_speculation_miss_event(),
            make_speculation_miss_event(),
            make_speculation_miss_event(),
        ]);

        let result = mape.cycle_once().await;
        assert!(result.is_some());

        let (remediation, rem_result, _gen) = result.unwrap();
        assert_eq!(remediation.kind(), RemediationKind::ReduceDraftMax);
        assert!(rem_result.is_success());
        assert_eq!(ctx.draft_max(), 8, "draft-max should be reduced from 16 to 8");
    }

    #[tokio::test]
    async fn test_cycle_once_verification_failure() {
        let peer = NodeId(Ulid::new());
        let (mape, ctx) = make_loop(vec![make_verification_failed_event(peer)]);

        let result = mape.cycle_once().await;
        assert!(result.is_some());

        let (remediation, rem_result, gen_test) = result.unwrap();
        assert_eq!(remediation.kind(), RemediationKind::TriggerKaniReverification);
        assert!(rem_result.is_success());
        assert_eq!(ctx.kani_queue_len(), 1, "target should be queued for Kani");
        // Verification failures generate Kani harnesses.
        assert!(gen_test.is_some());
        let test = gen_test.unwrap();
        assert!(test.source.contains("#[kani::proof]"));
    }

    #[tokio::test]
    async fn test_cycle_once_records_action_in_knowledge() {
        let (mape, _ctx) = make_loop(vec![make_thermal_event(75.0, true)]);

        let _ = mape.cycle_once().await;

        // The knowledge base should have a recorded action.
        let actions = mape.knowledge().actions();
        assert_eq!(actions.len(), 1, "should record one autonomic action");
        assert!(actions[0].remediation.contains("reduce_prefetch_depth"));
    }

    #[tokio::test]
    async fn test_cycle_once_no_pattern_returns_none() {
        // Inject an event that doesn't trigger any pattern (low temp, not throttled).
        let (mape, _ctx) = make_loop(vec![make_thermal_event(45.0, false)]);
        let result = mape.cycle_once().await;
        assert!(result.is_none(), "should not produce a remediation when no pattern is detected");
    }

    #[tokio::test]
    async fn test_cycle_once_skipped_remmediation_does_not_generate_test() {
        // Set the context to prefetch_depth=0 so ReducePrefetchDepth is Skipped.
        let (tx, rx) = mpsc::channel(256);
        let _ = tx.try_send(make_thermal_event(75.0, true));
        let knowledge = Arc::new(AutonomicKnowledge::new());
        let test_gen = TestGenerator::new();
        let ctx = Arc::new(RemediationContext::with_initial_state(0, 16));
        let mape = MapeKLoop::new(
            MapeKConfig::default(),
            rx,
            knowledge,
            test_gen,
            ctx.clone(),
        );

        let result = mape.cycle_once().await;
        assert!(result.is_some());

        let (_rem, rem_result, gen_test) = result.unwrap();
        assert!(rem_result.is_skipped(), "should be skipped at depth 0");
        assert!(gen_test.is_none(), "should not generate a test for a skipped remediation");
    }
}
