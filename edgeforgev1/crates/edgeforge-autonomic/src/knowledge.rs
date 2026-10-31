// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::remediation::RemediationResult;
use edgeforge_core::ControlEvent;
use parking_lot::RwLock;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureKind {
    ThermalThrottle,
    PeerDisconnect,
    CorruptExpert,
    SpeculationMissSpike,
    VerificationFailure,
}
#[derive(Clone, Debug)]
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
pub struct AutonomicAction {
    pub remediation: String,
    pub target: String,
    pub result: RemediationResult,
    pub timestamp_ms: i64,
}
pub struct AutonomicKnowledge {
    events: RwLock<Vec<ControlEvent>>,
    patterns: RwLock<HashMap<FailureKind, FailurePattern>>,
    actions: RwLock<Vec<AutonomicAction>>,
}
impl AutonomicKnowledge {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            patterns: RwLock::new(HashMap::new()),
            actions: RwLock::new(Vec::new()),
        }
    }
    pub fn record_event(&self, event: ControlEvent) {
        let mut e = self.events.write();
        e.push(event);
        let len = e.len();
        if len > 10_000 {
            e.drain(..len - 10_000);
        }
    }
    pub fn detect_patterns(
        &self,
        _class: &[(String, f64, crate::VariationKind)],
    ) -> Vec<FailurePattern> {
        Vec::new()
    }
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
