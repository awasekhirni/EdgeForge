// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # ControlEvent — Unified Event Bus Schema (ADR-019)
//!
//! ## Purpose
//! Every state change in the system emits a ControlEvent. The streaming graph
//! (L3.5), the crowd reputation graph (L2.5), the OTLP exporter (opt-in), and
//! the benchmark harness all subscribe to this stream.
//!
//! ## What This File Does
//! - Defines `ControlEvent` struct (timestamp_ms + kind)
//! - Defines `ControlEventKind` enum with 8 variants:
//!   RouterDecision, IoCompletion, PeerHeartbeat, PeerLost, ThermalSample,
//!   SpeculationVerdict, TokenGenerated, VerificationVerdict
//! - Defines `SpeculationHorizon` (H1/H2/H3)
//! - Defines `CrowdTier` (Trusted/Cooperative/Open)
//! - Defines `VerificationOutcome` (Verified/Pending/Failed/Corrected)
//!
//! ## Dependencies
//! - `serde` — Serialize for JSON encoding
//! - `crate::types` — ExpertId, ExpertSource, JobId, LayerIdx, ModelId, NodeId
//!
//! ## Output
//! Serializable event structs consumed by the streaming graph and MAPE-K loop.
//!
//! ## Architectural Invariants
//! 1. Events are immutable and timestamped (unix ms).
//! 2. Events NEVER carry prompt content (R-14, R-20).
//! 3. Events are lossy under load (bounded channel, drops oldest).

use crate::types::{ExpertId, ExpertSource, JobId, LayerIdx, ModelId, NodeId};
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
/// `ControlEvent` — see module documentation for purpose and usage.
pub struct ControlEvent {
    pub timestamp_ms: i64,
    pub kind: ControlEventKind,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// `ControlEventKind` enumeration — see module documentation for variants.
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
/// `SpeculationHorizon` enumeration — see module documentation for variants.
pub enum SpeculationHorizon {
    H1,
    H2,
    H3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// `CrowdTier` enumeration — see module documentation for variants.
pub enum CrowdTier {
    Trusted,
    Cooperative,
    Open,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// `VerificationOutcome` enumeration — see module documentation for variants.
pub enum VerificationOutcome {
    Verified,
    Pending,
    Failed,
    Corrected,
}
