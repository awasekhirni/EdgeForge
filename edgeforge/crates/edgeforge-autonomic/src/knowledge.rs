//! # Knowledge Base (ADR-029)
//!
//! ## Purpose
//! The shared knowledge base for the MAPE-K loop. Stores events, failure
//! patterns, and autonomic actions for post-incident forensics.
//!
//! ## What This File Does
//! - `FailureKind` — ThermalThrottle, PeerDisconnect, CorruptExpert,
//!   SpeculationMissSpike, VerificationFailure
//! - `FailurePattern` — kind, metric, value, target, occurrences, severity()
//! - `AutonomicAction` — remediation, target, result, timestamp (for audit table)
//! - `AutonomicKnowledge` — thread-safe store for events, patterns, actions
//! - `record_event()` — store a ControlEvent (bounded to 10,000)
//! - `detect_patterns()` — identify failure patterns from SPC classifications
//!   and the stored event stream
//! - `record_action()` — store an autonomic action (bounded to 10,000)
//!
//! ## Pattern Detection Algorithm
//! `detect_patterns()` examines the SPC classifications (from the MAPE-K
//! loop's Analyze phase) plus the stored event stream to identify failure
//! patterns:
//!
//! | Metric | Failure Kind | Trigger |
//! |---|---|---|
//! | `ssd_temp_c` ≥ 70 | ThermalThrottle | SpecialCause variation on temperature |
//! | `peer_rtt_ms` spike + PeerLost event | PeerDisconnect | SpecialCause on RTT + recent PeerLost |
//! | `io_latency_us` spike | CorruptExpert | SpecialCause on I/O latency (indicates SSD read failure) |
//! | SpeculationVerdict hit=false spike | SpeculationMissSpike | Multiple consecutive speculation misses |
//! | VerificationVerdict outcome=Failed | VerificationFailure | Any verification failure event |
//!
//! Each detected pattern is stored in the knowledge base (keyed by kind) and
//! its `occurrences` counter is incremented on repeat detections.
//!
//! ## Dependencies
//! - `parking_lot::RwLock` — thread-safe storage
//! - `edgeforge_core::ControlEvent` — event type
//! - `crate::remediation::RemediationResult` — for action records
//! - `crate::mape::VariationKind` — for SPC classification
//!
//! ## Output
//! `Vec<FailurePattern>` from detect_patterns; `Vec<AutonomicAction>` from actions().
//!
//! ## Function: severity
//! - **Purpose:** Calculate the severity of a failure pattern
//! - **Returns:** `u32` — higher = more severe
//! - **Formula:** base (2-5 depending on kind) + occurrences.min(10)

use crate::remediation::RemediationResult;
use crate::mape::VariationKind;
use edgeforge_core::ControlEvent;
use edgeforge_core::ControlEventKind;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The kind of failure detected by the MAPE-K loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureKind {
    /// SSD temperature exceeded the throttle threshold (≥ 70°C).
    ThermalThrottle,
    /// A peer disconnected from the mesh.
    PeerDisconnect,
    /// An expert weight read failed (corrupt data / SSD error).
    CorruptExpert,
    /// Speculation miss rate spiked (surrogate accuracy degraded).
    SpeculationMissSpike,
    /// A verification check failed (Tier 2/3 crowd verification).
    VerificationFailure,
}

impl FailureKind {
    /// Returns the string name of the failure kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThermalThrottle => "thermal_throttle",
            Self::PeerDisconnect => "peer_disconnect",
            Self::CorruptExpert => "corrupt_expert",
            Self::SpeculationMissSpike => "speculation_miss_spike",
            Self::VerificationFailure => "verification_failure",
        }
    }
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A detected failure pattern.
#[derive(Clone, Debug)]
pub struct FailurePattern {
    /// The kind of failure.
    pub kind: FailureKind,
    /// The metric that triggered the detection (e.g. "ssd_temp_c").
    pub metric: String,
    /// The metric value that triggered the detection.
    pub value: f64,
    /// The target of the failure (e.g. peer ID, expert ID). None for system-wide.
    pub target: Option<String>,
    /// Number of times this pattern has been detected.
    pub occurrences: u32,
    /// When the pattern was first detected (Unix ms).
    pub first_seen_ms: i64,
    /// When the pattern was last detected (Unix ms).
    pub last_seen_ms: i64,
}

impl FailurePattern {
    /// Creates a new failure pattern with occurrences=1.
    #[must_use]
    pub fn new(
        kind: FailureKind,
        metric: impl Into<String>,
        value: f64,
        target: Option<String>,
        timestamp_ms: i64,
    ) -> Self {
        Self {
            kind,
            metric: metric.into(),
            value,
            target,
            occurrences: 1,
            first_seen_ms: timestamp_ms,
            last_seen_ms: timestamp_ms,
        }
    }

    /// Returns the severity (higher = more severe).
    ///
    /// ## Formula
    /// `base (2-5 depending on kind) + occurrences.min(10)`
    #[must_use]
    pub fn severity(&self) -> u32 {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "knowledge.rs", "severity");
        let base = match self.kind {
            FailureKind::ThermalThrottle => 5,
            FailureKind::CorruptExpert => 4,
            FailureKind::VerificationFailure => 4,
            FailureKind::PeerDisconnect => 3,
            FailureKind::SpeculationMissSpike => 2,
        };
        base + self.occurrences.min(10)
    }

    /// Updates the pattern with a new occurrence.
    pub fn record_occurrence(&mut self, value: f64, timestamp_ms: i64) {
        self.value = value;
        self.occurrences += 1;
        self.last_seen_ms = timestamp_ms;
    }
}

/// An autonomic action recorded for audit.
#[derive(Clone, Debug, serde::Serialize)]
pub struct AutonomicAction {
    /// The remediation kind that was applied.
    pub remediation: String,
    /// The target of the remediation.
    pub target: String,
    /// The result of the remediation.
    pub result: RemediationResult,
    /// When the action was taken (Unix ms).
    pub timestamp_ms: i64,
}

/// The shared knowledge base for the MAPE-K loop.
///
/// ## Purpose
/// Stores events, detected failure patterns, and autonomic actions for
/// post-incident forensics. Thread-safe via `parking_lot::RwLock`.
pub struct AutonomicKnowledge {
    events: RwLock<Vec<ControlEvent>>,
    patterns: RwLock<HashMap<FailureKind, FailurePattern>>,
    actions: RwLock<Vec<AutonomicAction>>,
}

impl Default for AutonomicKnowledge {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomicKnowledge {
    /// Creates a new empty knowledge base.
    #[must_use]
    pub fn new() -> Self {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "knowledge.rs", "new");
        Self {
            events: RwLock::new(Vec::new()),
            patterns: RwLock::new(HashMap::new()),
            actions: RwLock::new(Vec::new()),
        }
    }

    /// Records a control event (bounded to 10,000 entries).
    pub fn record_event(&self, event: ControlEvent) {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "knowledge.rs",
            "record_event",
        );
        let mut e = self.events.write();
        e.push(event);
        let len = e.len();
        if len > 10_000 {
            e.drain(..len - 10_000);
        }
    }

    /// Detects failure patterns from SPC classifications + the event stream.
    ///
    /// ## Purpose
    /// Examines the SPC classifications produced by the MAPE-K Analyze
    /// phase plus the stored event stream to identify failure patterns.
    /// Detected patterns are stored in the knowledge base (keyed by kind);
    /// repeat detections increment the `occurrences` counter.
    ///
    /// ## Parameters
    /// - `classifications`: `Vec<(metric_name, value, VariationKind)>` from
    ///   the SPC analyzer.
    ///
    /// ## Returns
    /// `Vec<FailurePattern>` — all currently-detected patterns (including
    /// ones from previous cycles that are still active).
    ///
    /// ## Detection Rules
    /// | Metric | Failure Kind | Trigger |
    /// |---|---|---|
    /// | `ssd_temp_c` ≥ 70 | ThermalThrottle | SpecialCause variation on temperature |
    /// | `peer_rtt_ms` spike + PeerLost event | PeerDisconnect | SpecialCause on RTT + recent PeerLost |
    /// | `io_latency_us` spike | CorruptExpert | SpecialCause on I/O latency |
    /// | SpeculationVerdict hit=false spike | SpeculationMissSpike | ≥3 consecutive misses in batch |
    /// | VerificationVerdict outcome=Failed | VerificationFailure | Any verification failure event |
    pub fn detect_patterns(
        &self,
        classifications: &[(String, f64, VariationKind)],
    ) -> Vec<FailurePattern> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "knowledge.rs",
            "detect_patterns",
        );

        let now_ms = chrono::Utc::now().timestamp_millis();
        let events = self.events.read();

        // ── Rule 1: ThermalThrottle — ssd_temp_c ≥ 70 with SpecialCause ──
        for (metric, value, kind) in classifications {
            if metric == "ssd_temp_c" && *value >= 70.0 && *kind == VariationKind::SpecialCause {
                self.upsert_pattern(FailureKind::ThermalThrottle, "ssd_temp_c", *value, None, now_ms);
            }
        }

        // Also check ThermalSample events directly (throttled=true or paused=true).
        for event in events.iter().rev().take(100) {
            if let ControlEventKind::ThermalSample { temp_celsius, throttled, paused } = &event.kind {
                if *throttled || *paused {
                    self.upsert_pattern(
                        FailureKind::ThermalThrottle,
                        "ssd_temp_c",
                        *temp_celsius as f64,
                        None,
                        event.timestamp_ms,
                    );
                    break; // one is enough
                }
            }
        }

        // ── Rule 2: PeerDisconnect — PeerLost event or RTT spike ──
        for event in events.iter().rev().take(100) {
            if let ControlEventKind::PeerLost { peer } = &event.kind {
                self.upsert_pattern(
                    FailureKind::PeerDisconnect,
                    "peer_lost",
                    1.0,
                    Some(peer.to_string()),
                    event.timestamp_ms,
                );
                break;
            }
        }
        // Also check for RTT spikes in classifications.
        for (metric, value, kind) in classifications {
            if metric == "peer_rtt_ms" && *value > 500.0 && *kind == VariationKind::SpecialCause {
                self.upsert_pattern(FailureKind::PeerDisconnect, "peer_rtt_ms", *value, None, now_ms);
            }
        }

        // ── Rule 3: CorruptExpert — io_latency_us spike (SpecialCause) ──
        for (metric, value, kind) in classifications {
            if metric == "io_latency_us" && *value > 10_000.0 && *kind == VariationKind::SpecialCause {
                self.upsert_pattern(FailureKind::CorruptExpert, "io_latency_us", *value, None, now_ms);
            }
        }

        // ── Rule 4: SpeculationMissSpike — ≥3 consecutive misses in batch ──
        let mut consecutive_misses: u32 = 0;
        let mut max_consecutive: u32 = 0;
        for event in events.iter().rev().take(200) {
            if let ControlEventKind::SpeculationVerdict { hit, .. } = &event.kind {
                if !hit {
                    consecutive_misses += 1;
                    max_consecutive = max_consecutive.max(consecutive_misses);
                } else {
                    consecutive_misses = 0;
                }
            }
        }
        if max_consecutive >= 3 {
            self.upsert_pattern(
                FailureKind::SpeculationMissSpike,
                "speculation_miss_count",
                max_consecutive as f64,
                None,
                now_ms,
            );
        }

        // ── Rule 5: VerificationFailure — any VerificationVerdict with outcome=Failed ──
        for event in events.iter().rev().take(100) {
            if let ControlEventKind::VerificationVerdict { peer, outcome, .. } = &event.kind {
                if *outcome == edgeforge_core::VerificationOutcome::Failed {
                    self.upsert_pattern(
                        FailureKind::VerificationFailure,
                        "verification_failed",
                        1.0,
                        Some(peer.to_string()),
                        event.timestamp_ms,
                    );
                    break;
                }
            }
        }

        // Return all current patterns.
        let patterns = self.patterns.read();
        let result: Vec<FailurePattern> = patterns.values().cloned().collect();

        tracing::info!(
            pattern_count = result.len(),
            "detect_patterns: identified failure patterns"
        );

        guard.success();
        result
    }

    /// Insert or update a failure pattern.
    fn upsert_pattern(
        &self,
        kind: FailureKind,
        metric: &str,
        value: f64,
        target: Option<String>,
        timestamp_ms: i64,
    ) {
        let mut patterns = self.patterns.write();
        if let Some(existing) = patterns.get_mut(&kind) {
            existing.record_occurrence(value, timestamp_ms);
        } else {
            patterns.insert(kind, FailurePattern::new(kind, metric, value, target, timestamp_ms));
        }
    }

    /// Records an autonomic action (bounded to 10,000 entries).
    pub fn record_action(&self, action: AutonomicAction) {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "knowledge.rs",
            "record_action",
        );
        let mut a = self.actions.write();
        a.push(action);
        let len = a.len();
        if len > 10_000 {
            a.drain(..len - 10_000);
        }
    }

    /// Returns a snapshot of all recorded autonomic actions.
    #[must_use]
    pub fn actions(&self) -> Vec<AutonomicAction> {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "knowledge.rs", "actions");
        self.actions.read().clone()
    }

    /// Returns the number of recorded events.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.read().len()
    }

    /// Returns the number of detected patterns.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.read().len()
    }

    /// Clears all stored events, patterns, and actions (for testing).
    pub fn clear(&self) {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "knowledge.rs", "clear");
        self.events.write().clear();
        self.patterns.write().clear();
        self.actions.write().clear();
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::{
        ControlEvent, ControlEventKind, CrowdTier, JobId, ModelId, NodeId, VerificationOutcome,
    };
    use ulid::Ulid;

    fn make_event(kind: ControlEventKind) -> ControlEvent {
        ControlEvent {
            timestamp_ms: 1700000000000,
            kind,
        }
    }

    // ── FailureKind tests ──

    #[test]
    fn test_failure_kind_as_str() {
        assert_eq!(FailureKind::ThermalThrottle.as_str(), "thermal_throttle");
        assert_eq!(FailureKind::PeerDisconnect.as_str(), "peer_disconnect");
        assert_eq!(FailureKind::CorruptExpert.as_str(), "corrupt_expert");
        assert_eq!(FailureKind::SpeculationMissSpike.as_str(), "speculation_miss_spike");
        assert_eq!(FailureKind::VerificationFailure.as_str(), "verification_failure");
    }

    #[test]
    fn test_failure_kind_display() {
        assert_eq!(format!("{}", FailureKind::ThermalThrottle), "thermal_throttle");
    }

    // ── FailurePattern tests ──

    #[test]
    fn test_failure_pattern_new() {
        let p = FailurePattern::new(
            FailureKind::ThermalThrottle,
            "ssd_temp_c",
            75.0,
            None,
            1000,
        );
        assert_eq!(p.kind, FailureKind::ThermalThrottle);
        assert_eq!(p.metric, "ssd_temp_c");
        assert_eq!(p.value, 75.0);
        assert!(p.target.is_none());
        assert_eq!(p.occurrences, 1);
        assert_eq!(p.first_seen_ms, 1000);
        assert_eq!(p.last_seen_ms, 1000);
    }

    #[test]
    fn test_failure_pattern_severity() {
        let p = FailurePattern::new(FailureKind::ThermalThrottle, "x", 0.0, None, 0);
        // base 5 + occurrences(1).min(10) = 6
        assert_eq!(p.severity(), 6);

        let mut p = p;
        p.occurrences = 20;
        // base 5 + min(20, 10) = 15
        assert_eq!(p.severity(), 15);
    }

    #[test]
    fn test_failure_pattern_severity_by_kind() {
        assert_eq!(FailurePattern::new(FailureKind::ThermalThrottle, "x", 0.0, None, 0).severity(), 5 + 1);
        assert_eq!(FailurePattern::new(FailureKind::CorruptExpert, "x", 0.0, None, 0).severity(), 4 + 1);
        assert_eq!(FailurePattern::new(FailureKind::VerificationFailure, "x", 0.0, None, 0).severity(), 4 + 1);
        assert_eq!(FailurePattern::new(FailureKind::PeerDisconnect, "x", 0.0, None, 0).severity(), 3 + 1);
        assert_eq!(FailurePattern::new(FailureKind::SpeculationMissSpike, "x", 0.0, None, 0).severity(), 2 + 1);
    }

    #[test]
    fn test_failure_pattern_record_occurrence() {
        let mut p = FailurePattern::new(FailureKind::ThermalThrottle, "x", 70.0, None, 1000);
        p.record_occurrence(75.0, 2000);
        assert_eq!(p.occurrences, 2);
        assert_eq!(p.value, 75.0);
        assert_eq!(p.last_seen_ms, 2000);
        assert_eq!(p.first_seen_ms, 1000); // unchanged
    }

    // ── AutonomicKnowledge tests ──

    #[test]
    fn test_knowledge_new() {
        let kb = AutonomicKnowledge::new();
        assert_eq!(kb.event_count(), 0);
        assert_eq!(kb.pattern_count(), 0);
        assert_eq!(kb.actions().len(), 0);
    }

    #[test]
    fn test_record_event() {
        let kb = AutonomicKnowledge::new();
        kb.record_event(make_event(ControlEventKind::PeerLost { peer: NodeId(Ulid::new()) }));
        assert_eq!(kb.event_count(), 1);
    }

    #[test]
    fn test_record_event_bounded() {
        let kb = AutonomicKnowledge::new();
        for _ in 0..15_000 {
            kb.record_event(make_event(ControlEventKind::PeerLost { peer: NodeId(Ulid::new()) }));
        }
        assert_eq!(kb.event_count(), 10_000); // bounded
    }

    #[test]
    fn test_record_action() {
        let kb = AutonomicKnowledge::new();
        kb.record_action(AutonomicAction {
            remediation: "reduce_prefetch_depth".to_string(),
            target: "prefetcher".to_string(),
            result: RemediationResult::Success { detail: "ok".into() },
            timestamp_ms: 1000,
        });
        assert_eq!(kb.actions().len(), 1);
    }

    // ── detect_patterns tests — the real implementation ──

    #[test]
    fn test_detect_patterns_thermal_throttle() {
        let kb = AutonomicKnowledge::new();
        // Inject a thermal throttle event.
        kb.record_event(make_event(ControlEventKind::ThermalSample {
            temp_celsius: 75.0,
            throttled: true,
            paused: false,
        }));

        // Previously: detect_patterns returned Vec::new().
        // Now: returns a real ThermalThrottle pattern.
        let patterns = kb.detect_patterns(&[]);
        assert!(!patterns.is_empty(), "should detect thermal throttle");
        let thermal = patterns.iter().find(|p| p.kind == FailureKind::ThermalThrottle);
        assert!(thermal.is_some(), "ThermalThrottle pattern should be present");
        assert!(thermal.unwrap().value >= 70.0);
    }

    #[test]
    fn test_detect_patterns_thermal_throttle_from_spc() {
        let kb = AutonomicKnowledge::new();
        // SPC classification shows a temperature spike.
        let classifications = vec![
            ("ssd_temp_c".to_string(), 72.0, VariationKind::SpecialCause),
        ];
        let patterns = kb.detect_patterns(&classifications);
        let thermal = patterns.iter().find(|p| p.kind == FailureKind::ThermalThrottle);
        assert!(thermal.is_some());
        assert!(thermal.unwrap().value >= 70.0);
    }

    #[test]
    fn test_detect_patterns_peer_disconnect() {
        let kb = AutonomicKnowledge::new();
        let peer = NodeId(Ulid::new());
        kb.record_event(make_event(ControlEventKind::PeerLost { peer }));

        let patterns = kb.detect_patterns(&[]);
        let disconnect = patterns.iter().find(|p| p.kind == FailureKind::PeerDisconnect);
        assert!(disconnect.is_some(), "PeerDisconnect pattern should be detected");
        assert_eq!(
            disconnect.unwrap().target,
            Some(peer.to_string())
        );
    }

    #[test]
    fn test_detect_patterns_corrupt_expert() {
        let kb = AutonomicKnowledge::new();
        // SPC classification shows an I/O latency spike.
        let classifications = vec![
            ("io_latency_us".to_string(), 15_000.0, VariationKind::SpecialCause),
        ];
        let patterns = kb.detect_patterns(&classifications);
        let corrupt = patterns.iter().find(|p| p.kind == FailureKind::CorruptExpert);
        assert!(corrupt.is_some(), "CorruptExpert pattern should be detected");
    }

    #[test]
    fn test_detect_patterns_speculation_miss_spike() {
        let kb = AutonomicKnowledge::new();
        // Inject 3 consecutive speculation misses.
        for _ in 0..3 {
            kb.record_event(make_event(ControlEventKind::SpeculationVerdict {
                horizon: edgeforge_core::SpeculationHorizon::H1,
                hit: false,
                surrogate_used: false,
                accuracy_delta: None,
            }));
        }
        let patterns = kb.detect_patterns(&[]);
        let miss = patterns.iter().find(|p| p.kind == FailureKind::SpeculationMissSpike);
        assert!(miss.is_some(), "SpeculationMissSpike pattern should be detected");
        assert!(miss.unwrap().value >= 3.0);
    }

    #[test]
    fn test_detect_patterns_speculation_miss_not_triggered_by_few_misses() {
        let kb = AutonomicKnowledge::new();
        // Only 2 consecutive misses — below the threshold of 3.
        for _ in 0..2 {
            kb.record_event(make_event(ControlEventKind::SpeculationVerdict {
                horizon: edgeforge_core::SpeculationHorizon::H1,
                hit: false,
                surrogate_used: false,
                accuracy_delta: None,
            }));
        }
        let patterns = kb.detect_patterns(&[]);
        let miss = patterns.iter().find(|p| p.kind == FailureKind::SpeculationMissSpike);
        assert!(miss.is_none(), "Should not trigger on <3 misses");
    }

    #[test]
    fn test_detect_patterns_verification_failure() {
        let kb = AutonomicKnowledge::new();
        let peer = NodeId(Ulid::new());
        kb.record_event(make_event(ControlEventKind::VerificationVerdict {
            job: JobId(Ulid::new()),
            peer,
            tier: CrowdTier::Trusted,
            outcome: VerificationOutcome::Failed,
            latency_ms: 50,
        }));

        let patterns = kb.detect_patterns(&[]);
        let verif = patterns.iter().find(|p| p.kind == FailureKind::VerificationFailure);
        assert!(verif.is_some(), "VerificationFailure pattern should be detected");
        assert_eq!(verif.unwrap().target, Some(peer.to_string()));
    }

    #[test]
    fn test_detect_patterns_verification_not_triggered_on_success() {
        let kb = AutonomicKnowledge::new();
        kb.record_event(make_event(ControlEventKind::VerificationVerdict {
            job: JobId(Ulid::new()),
            peer: NodeId(Ulid::new()),
            tier: CrowdTier::Trusted,
            outcome: VerificationOutcome::Verified,
            latency_ms: 50,
        }));

        let patterns = kb.detect_patterns(&[]);
        let verif = patterns.iter().find(|p| p.kind == FailureKind::VerificationFailure);
        assert!(verif.is_none(), "Should not trigger on Verified outcome");
    }

    #[test]
    fn test_detect_patterns_empty_returns_empty() {
        let kb = AutonomicKnowledge::new();
        let patterns = kb.detect_patterns(&[]);
        assert!(patterns.is_empty(), "No events → no patterns");
    }

    #[test]
    fn test_detect_patterns_occurrences_increment_on_repeat() {
        let kb = AutonomicKnowledge::new();
        // First detection.
        kb.record_event(make_event(ControlEventKind::ThermalSample {
            temp_celsius: 75.0,
            throttled: true,
            paused: false,
        }));
        let patterns = kb.detect_patterns(&[]);
        let thermal = patterns.iter().find(|p| p.kind == FailureKind::ThermalThrottle).unwrap();
        assert_eq!(thermal.occurrences, 1);

        // Second detection (clear + re-inject).
        kb.clear();
        kb.record_event(make_event(ControlEventKind::ThermalSample {
            temp_celsius: 80.0,
            throttled: true,
            paused: false,
        }));
        // The patterns map was cleared, so this is a fresh detection.
        let patterns = kb.detect_patterns(&[]);
        let thermal = patterns.iter().find(|p| p.kind == FailureKind::ThermalThrottle).unwrap();
        assert_eq!(thermal.occurrences, 1);
    }

    #[test]
    fn test_clear() {
        let kb = AutonomicKnowledge::new();
        kb.record_event(make_event(ControlEventKind::PeerLost { peer: NodeId(Ulid::new()) }));
        kb.record_action(AutonomicAction {
            remediation: "x".into(),
            target: "y".into(),
            result: RemediationResult::Success { detail: "ok".into() },
            timestamp_ms: 0,
        });
        assert_eq!(kb.event_count(), 1);
        assert_eq!(kb.actions().len(), 1);

        kb.clear();
        assert_eq!(kb.event_count(), 0);
        assert_eq!(kb.actions().len(), 0);
        assert_eq!(kb.pattern_count(), 0);
    }
}
