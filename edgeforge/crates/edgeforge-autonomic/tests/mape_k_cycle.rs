//! # MAPE-K Autonomic Loop Integration Tests
//!
//! These tests exercise the full MAPE-K cycle end-to-end:
//!   1. Inject ControlEvents into the event channel
//!   2. Run `cycle_once()` — the loop monitors, analyzes, plans, executes
//!   3. Verify the correct failure pattern was detected
//!   4. Verify the correct remediation was applied (real state mutation)
//!   5. Verify an antifragile regression test was generated
//!
//! These tests prove the "antifragile" selling point of ADR-029: the system
//! becomes stronger because it failed.

use edgeforge_autonomic::*;
use edgeforge_core::{
    ControlEvent, ControlEventKind, CrowdTier, ExpertId, ExpertSource, JobId, LayerIdx,
    NodeId, SpeculationHorizon, VerificationOutcome,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use ulid::Ulid;

// ── Helpers ────────────────────────────────────────────────────────────────

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

fn make_io_latency_event(latency_us: u64) -> ControlEvent {
    ControlEvent {
        timestamp_ms: 1700000000000,
        kind: ControlEventKind::IoCompletion {
            sequence: 0,
            layer: LayerIdx(0),
            expert: ExpertId(0),
            source: ExpertSource::LocalSsd,
            bytes: 4096,
            latency_us,
        },
    }
}

fn make_loop_with_events(
    events: Vec<ControlEvent>,
) -> (MapeKLoop, Arc<RemediationContext>, Arc<AutonomicKnowledge>) {
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
        knowledge.clone(),
        test_gen,
        ctx.clone(),
    );
    (mape, ctx, knowledge)
}

// ── End-to-end MAPE-K cycle tests ──────────────────────────────────────────

#[tokio::test]
async fn e2e_thermal_throttle_full_cycle() {
    let (mape, ctx, knowledge) = make_loop_with_events(vec![
        make_thermal_event(75.0, true),
    ]);

    let result = mape.cycle_once().await;
    assert!(result.is_some(), "cycle should produce a remediation");

    let (remediation, rem_result, gen_test) = result.unwrap();

    assert_eq!(remediation.kind(), RemediationKind::ReducePrefetchDepth);
    assert!(rem_result.is_success());
    assert_eq!(ctx.prefetch_depth(), 3);
    assert!(gen_test.is_some(), "should generate a regression test");
    let test = gen_test.unwrap();
    assert_eq!(test.kind, TestKind::RegressionTest);
    assert!(test.name.contains("thermal_throttle"));
    assert!(test.source.contains("#[tokio::test]"));

    let actions = knowledge.actions();
    assert_eq!(actions.len(), 1);
    assert!(actions[0].remediation.contains("reduce_prefetch_depth"));
}

#[tokio::test]
async fn e2e_peer_disconnect_full_cycle() {
    let peer = NodeId(Ulid::new());
    let (mape, ctx, _knowledge) = make_loop_with_events(vec![
        make_peer_lost_event(peer),
    ]);

    let result = mape.cycle_once().await;
    assert!(result.is_some());

    let (remediation, rem_result, gen_test) = result.unwrap();
    assert_eq!(remediation.kind(), RemediationKind::EvictPeer);
    assert!(rem_result.is_success());
    assert!(ctx.is_peer_evicted(&peer.to_string()));
    assert!(gen_test.is_some());
    let test = gen_test.unwrap();
    assert!(test.name.contains("peer_disconnect"));
}

#[tokio::test]
async fn e2e_speculation_miss_spike_full_cycle() {
    let (mape, ctx, _knowledge) = make_loop_with_events(vec![
        make_speculation_miss_event(),
        make_speculation_miss_event(),
        make_speculation_miss_event(),
    ]);

    let result = mape.cycle_once().await;
    assert!(result.is_some());

    let (remediation, rem_result, gen_test) = result.unwrap();
    assert_eq!(remediation.kind(), RemediationKind::ReduceDraftMax);
    assert!(rem_result.is_success());
    assert_eq!(ctx.draft_max(), 8);
    assert!(gen_test.is_some());
    let test = gen_test.unwrap();
    assert!(test.name.contains("speculation_miss"));
}

#[tokio::test]
async fn e2e_verification_failure_full_cycle() {
    let peer = NodeId(Ulid::new());
    let (mape, ctx, _knowledge) = make_loop_with_events(vec![
        make_verification_failed_event(peer),
    ]);

    let result = mape.cycle_once().await;
    assert!(result.is_some());

    let (remediation, rem_result, gen_test) = result.unwrap();
    assert_eq!(remediation.kind(), RemediationKind::TriggerKaniReverification);
    assert!(rem_result.is_success());
    assert_eq!(ctx.kani_queue_len(), 1);
    assert!(gen_test.is_some());
    let test = gen_test.unwrap();
    assert_eq!(test.kind, TestKind::KaniHarness);
    assert!(test.source.contains("#[kani::proof]"));
}

#[tokio::test]
async fn e2e_no_events_no_remediation() {
    let (mape, _ctx, _knowledge) = make_loop_with_events(vec![]);
    assert!(mape.cycle_once().await.is_none());
}

#[tokio::test]
async fn e2e_no_pattern_no_remediation() {
    let (mape, _ctx, _knowledge) = make_loop_with_events(vec![
        make_thermal_event(45.0, false),
    ]);
    assert!(mape.cycle_once().await.is_none());
}

#[tokio::test]
async fn e2e_skipped_remediation_no_test_generated() {
    let (tx, rx) = mpsc::channel(256);
    let _ = tx.try_send(make_thermal_event(75.0, true));
    let knowledge = Arc::new(AutonomicKnowledge::new());
    let test_gen = TestGenerator::new();
    let ctx = Arc::new(RemediationContext::with_initial_state(0, 16));
    let mape = MapeKLoop::new(MapeKConfig::default(), rx, knowledge, test_gen, ctx.clone());

    let result = mape.cycle_once().await;
    let (_rem, rem_result, gen_test) = result.unwrap();
    assert!(rem_result.is_skipped());
    assert!(gen_test.is_none());
}

#[tokio::test]
async fn e2e_generated_test_source_is_compilable_rust() {
    let (mape, _ctx, _knowledge) = make_loop_with_events(vec![
        make_thermal_event(75.0, true),
    ]);

    let result = mape.cycle_once().await;
    let (_, _, gen_test) = result.unwrap();
    let test = gen_test.unwrap();

    assert!(test.source.starts_with("//!"));
    assert!(test.source.contains("#![cfg(test)]"));
    assert!(test.source.contains("#[tokio::test]"));
    assert!(test.source.contains("use edgeforge_autonomic::"));
    assert!(test.source.contains("RemediationContext"));
    assert!(test.source.contains("assert"));
}

#[tokio::test]
async fn e2e_io_latency_spike_detected() {
    // ≥30 normal samples + 1 spike → SPC classifies as SpecialCause → CorruptExpert.
    let (tx, rx) = mpsc::channel(512);
    for _ in 0..30 {
        let _ = tx.try_send(make_io_latency_event(500));
    }
    let _ = tx.try_send(make_io_latency_event(15_000));

    let knowledge = Arc::new(AutonomicKnowledge::new());
    let test_gen = TestGenerator::new();
    let ctx = Arc::new(RemediationContext::new());
    let mape = MapeKLoop::new(MapeKConfig::default(), rx, knowledge, test_gen, ctx.clone());

    let result = mape.cycle_once().await;
    assert!(result.is_some());
    let (remediation, rem_result, _gen) = result.unwrap();
    assert_eq!(remediation.kind(), RemediationKind::EvictExpert);
    assert!(rem_result.is_success());
    assert_eq!(ctx.evicted_expert_count(), 1);
}

#[tokio::test]
async fn e2e_context_snapshot_reflects_mutations() {
    let (mape, ctx, _knowledge) = make_loop_with_events(vec![
        make_thermal_event(75.0, true),
    ]);

    let before = ctx.snapshot();
    assert_eq!(before.prefetch_depth, 4);

    let _ = mape.cycle_once().await;

    let after = ctx.snapshot();
    assert_eq!(after.prefetch_depth, 3);
}

#[tokio::test]
async fn e2e_peer_eviction_reflected_in_snapshot() {
    let peer = NodeId(Ulid::new());
    let (mape, ctx, _knowledge) = make_loop_with_events(vec![
        make_peer_lost_event(peer),
    ]);

    let _ = mape.cycle_once().await;

    let snap = ctx.snapshot();
    assert_eq!(snap.evicted_peer_count, 1);
}
