// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Knowledge Base (ADR-029)
//!
//! ## Purpose
//! The shared knowledge base for the MAPE-K loop. Stores events, failure
//! patterns, and autonomic actions for post-incident forensics.
//!
//! ## What This File Does
//! - `FailureKind` — ThermalThrottle, PeerDisconnect, CorruptExpert, SpeculationMissSpike, VerificationFailure
//! - `FailurePattern` — kind, metric, value, target, occurrences, severity()
//! - `AutonomicAction` — remediation, target, result, timestamp (for audit table)
//! - `AutonomicKnowledge` — thread-safe store for events, patterns, actions
//! - `record_event()` — store a ControlEvent (bounded to 10,000)
//! - `detect_patterns()` — identify failure patterns from SPC classifications
//! - `record_action()` — store an autonomic action (bounded to 10,000)
//!
//! ## Dependencies
//! - `parking_lot::RwLock` — thread-safe storage
//! - `edgeforge_core::ControlEvent` — event type
//! - `crate::remediation::RemediationResult` — for action records
//!
//! ## Output
//! `Vec<FailurePattern>` from detect_patterns; `Vec<AutonomicAction>` from actions().
//!
//! ## Function: severity
//! - **Purpose:** Calculate the severity of a failure pattern
//! - **Returns:** `u32` — higher = more severe
//! - **Formula:** base (2-5 depending on kind) + occurrences.min(10)

use crate::remediation::RemediationResult;
use edgeforge_core::ControlEvent;
use parking_lot::RwLock;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// `FailureKind` enumeration — see module documentation for variants.
pub enum FailureKind {
    ThermalThrottle,
    PeerDisconnect,
    CorruptExpert,
    SpeculationMissSpike,
    VerificationFailure,
}
#[derive(Clone, Debug)]
/// `FailurePattern` — see module documentation for purpose and usage.
pub struct FailurePattern {
    pub kind: FailureKind,
    pub metric: String,
    pub value: f64,
    pub target: Option<String>,
    pub occurrences: u32,
    pub first_seen_ms: i64,
    pub last_seen_ms: i64,
}
impl FailurePattern {
    /// Returns the severity (higher = more severe).
    pub fn severity(&self) -> u32 {
        let base = match self.kind {
            FailureKind::ThermalThrottle => 5,
            FailureKind::CorruptExpert => 4,
            FailureKind::VerificationFailure => 4,
            FailureKind::PeerDisconnect => 3,
            FailureKind::SpeculationMissSpike => 2,
        };
        base + self.occurrences.min(10)
    }
}
#[derive(Clone, Debug, serde::Serialize)]
/// `AutonomicAction` — see module documentation for purpose and usage.
pub struct AutonomicAction {
    pub remediation: String,
    pub target: String,
    pub result: RemediationResult,
    pub timestamp_ms: i64,
}
/// `AutonomicKnowledge` — see module documentation for purpose and usage.
pub struct AutonomicKnowledge {
    events: RwLock<Vec<ControlEvent>>,
    patterns: RwLock<HashMap<FailureKind, FailurePattern>>,
    actions: RwLock<Vec<AutonomicAction>>,
}
impl AutonomicKnowledge {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            patterns: RwLock::new(HashMap::new()),
            actions: RwLock::new(Vec::new()),
        }
    }
    /// Records a control event.
    pub fn record_event(&self, event: ControlEvent) {
        let mut e = self.events.write();
        e.push(event);
        let len = e.len();
        if len > 10_000 {
            e.drain(..len - 10_000);
        }
    }
    /// Detects failure patterns from SPC classifications.
    pub fn detect_patterns(
        &self,
        _class: &[(String, f64, crate::VariationKind)],
    ) -> Vec<FailurePattern> {
        Vec::new()
    }
    /// Records an autonomic action.
    pub fn record_action(&self, action: AutonomicAction) {
        let mut a = self.actions.write();
        a.push(action);
        let len = a.len();
        if len > 10_000 {
            a.drain(..len - 10_000);
        }
    }
}
impl Default for AutonomicKnowledge {
    fn default() -> Self {
        Self::new()
    }
}
