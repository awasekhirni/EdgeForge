// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//
use crate::types::{ExpertId, ExpertSource, JobId, LayerIdx, ModelId, NodeId};
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct ControlEvent {
    pub timestamp_ms: i64,
    pub kind: ControlEventKind,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlEventKind {
    RouterDecision {
        sequence: u64,
        layer: LayerIdx,
        experts: Vec<ExpertId>,
    },
    IoCompletion {
        sequence: u64,
        layer: LayerIdx,
        expert: ExpertId,
        source: ExpertSource,
        bytes: u64,
        latency_us: u64,
    },
    PeerHeartbeat {
        peer: NodeId,
        rtt_ms: u32,
        thermal_celsius: f32,
        experts_resident: u32,
    },
    PeerLost {
        peer: NodeId,
    },
    ThermalSample {
        temp_celsius: f32,
        throttled: bool,
        paused: bool,
    },
    SpeculationVerdict {
        horizon: SpeculationHorizon,
        hit: bool,
        surrogate_used: bool,
        accuracy_delta: Option<f32>,
    },
    TokenGenerated {
        sequence: u64,
        job: JobId,
        model: ModelId,
        tokens_so_far: u32,
    },
    VerificationVerdict {
        job: JobId,
        peer: NodeId,
        tier: CrowdTier,
        outcome: VerificationOutcome,
        latency_ms: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeculationHorizon {
    H1,
    H2,
    H3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CrowdTier {
    Trusted,
    Cooperative,
    Open,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationOutcome {
    Verified,
    Pending,
    Failed,
    Corrected,
}
