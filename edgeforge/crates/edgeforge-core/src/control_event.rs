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
pub struct ControlEvent { pub timestamp_ms: i64, pub kind: ControlEventKind }
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// `ControlEventKind` enumeration — see module documentation for variants.
pub enum ControlEventKind {
    RouterDecision { sequence: u64, layer: LayerIdx, experts: Vec<ExpertId> },
    IoCompletion { sequence: u64, layer: LayerIdx, expert: ExpertId, source: ExpertSource, bytes: u64, latency_us: u64 },
    PeerHeartbeat { peer: NodeId, rtt_ms: u32, thermal_celsius: f32, experts_resident: u32 },
    PeerLost { peer: NodeId },
    ThermalSample { temp_celsius: f32, throttled: bool, paused: bool },
    SpeculationVerdict { horizon: SpeculationHorizon, hit: bool, surrogate_used: bool, accuracy_delta: Option<f32> },
    TokenGenerated { sequence: u64, job: JobId, model: ModelId, tokens_so_far: u32 },
    VerificationVerdict { job: JobId, peer: NodeId, tier: CrowdTier, outcome: VerificationOutcome, latency_ms: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// `SpeculationHorizon` enumeration — see module documentation for variants.
pub enum SpeculationHorizon { H1, H2, H3 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// `CrowdTier` enumeration — see module documentation for variants.
pub enum CrowdTier { Trusted, Cooperative, Open }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
/// `VerificationOutcome` enumeration — see module documentation for variants.
pub enum VerificationOutcome { Verified, Pending, Failed, Corrected }

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    #[test]
    fn test_control_event_construction() {
        let event = ControlEvent {
            timestamp_ms: 1700000000000,
            kind: ControlEventKind::PeerLost { peer: NodeId(Ulid::new()) },
        };
        assert_eq!(event.timestamp_ms, 1700000000000);
        assert!(matches!(event.kind, ControlEventKind::PeerLost { .. }));
    }

    #[test]
    fn test_router_decision_variant() {
        let event = ControlEventKind::RouterDecision {
            sequence: 1,
            layer: LayerIdx(0),
            experts: vec![ExpertId(0), ExpertId(1)],
        };
        assert!(matches!(event, ControlEventKind::RouterDecision { .. }));
    }

    #[test]
    fn test_io_completion_variant() {
        let event = ControlEventKind::IoCompletion {
            sequence: 1,
            layer: LayerIdx(0),
            expert: ExpertId(0),
            source: ExpertSource::LocalSsd,
            bytes: 4096,
            latency_us: 500,
        };
        assert!(matches!(event, ControlEventKind::IoCompletion { .. }));
    }

    #[test]
    fn test_peer_heartbeat_variant() {
        let event = ControlEventKind::PeerHeartbeat {
            peer: NodeId(Ulid::new()),
            rtt_ms: 25,
            thermal_celsius: 45.0,
            experts_resident: 10,
        };
        assert!(matches!(event, ControlEventKind::PeerHeartbeat { .. }));
    }

    #[test]
    fn test_thermal_sample_variant() {
        let event = ControlEventKind::ThermalSample {
            temp_celsius: 75.0,
            throttled: true,
            paused: false,
        };
        assert!(matches!(event, ControlEventKind::ThermalSample { .. }));
    }

    #[test]
    fn test_speculation_verdict_variant() {
        let event = ControlEventKind::SpeculationVerdict {
            horizon: SpeculationHorizon::H1,
            hit: true,
            surrogate_used: false,
            accuracy_delta: Some(0.95),
        };
        assert!(matches!(event, ControlEventKind::SpeculationVerdict { .. }));
    }

    #[test]
    fn test_token_generated_variant() {
        let event = ControlEventKind::TokenGenerated {
            sequence: 1,
            job: JobId(Ulid::new()),
            model: ModelId(Ulid::new()),
            tokens_so_far: 42,
        };
        assert!(matches!(event, ControlEventKind::TokenGenerated { .. }));
    }

    #[test]
    fn test_verification_verdict_variant() {
        let event = ControlEventKind::VerificationVerdict {
            job: JobId(Ulid::new()),
            peer: NodeId(Ulid::new()),
            tier: CrowdTier::Trusted,
            outcome: VerificationOutcome::Verified,
            latency_ms: 10,
        };
        assert!(matches!(event, ControlEventKind::VerificationVerdict { .. }));
    }

    #[test]
    fn test_speculation_horizon_equality() {
        assert_eq!(SpeculationHorizon::H1, SpeculationHorizon::H1);
        assert_ne!(SpeculationHorizon::H1, SpeculationHorizon::H2);
        assert_ne!(SpeculationHorizon::H2, SpeculationHorizon::H3);
    }

    #[test]
    fn test_crowd_tier_equality() {
        assert_eq!(CrowdTier::Trusted, CrowdTier::Trusted);
        assert_ne!(CrowdTier::Trusted, CrowdTier::Cooperative);
        assert_ne!(CrowdTier::Cooperative, CrowdTier::Open);
    }

    #[test]
    fn test_verification_outcome_equality() {
        assert_eq!(VerificationOutcome::Verified, VerificationOutcome::Verified);
        assert_ne!(VerificationOutcome::Verified, VerificationOutcome::Pending);
        assert_ne!(VerificationOutcome::Pending, VerificationOutcome::Failed);
        assert_ne!(VerificationOutcome::Failed, VerificationOutcome::Corrected);
    }

    #[test]
    fn test_control_event_serializes() {
        let event = ControlEvent {
            timestamp_ms: 0,
            kind: ControlEventKind::PeerLost { peer: NodeId(Ulid::new()) },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("peer_lost"));
    }

    #[test]
    fn test_speculation_horizon_serializes() {
        let json = serde_json::to_string(&SpeculationHorizon::H1).unwrap();
        assert_eq!(json, "\"h1\"");
    }

    #[test]
    fn test_crowd_tier_serializes() {
        let json = serde_json::to_string(&CrowdTier::Trusted).unwrap();
        assert_eq!(json, "\"trusted\"");
    }

    #[test]
    fn test_verification_outcome_serializes() {
        let json = serde_json::to_string(&VerificationOutcome::Verified).unwrap();
        assert_eq!(json, "\"verified\"");
    }
}
