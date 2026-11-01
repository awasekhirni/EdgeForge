#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-autonomic — L5.5 MAPE-K Autonomic Loop (ADR-029)
//!
//! ## Purpose
//! Monitor-Analyze-Plan-Execute over shared Knowledge. When a failure pattern
//! is resolved, the loop automatically generates a regression test — the
//! system becomes antifragile (stronger because it failed).
//!
//! ## What This File Does
//! This crate implements the full MAPE-K autonomic loop with real state
//! mutation, real pattern detection, and real test generation:
//! - `Remediation::execute()` mutates a `RemediationContext` (prefetch depth,
//!   evicted peers/experts, draft-max, Kani queue) — no more hardcoded
//!   "Success" strings
//! - `Knowledge::detect_patterns()` examines SPC classifications + events to
//!   detect 5 failure kinds (thermal throttle, peer disconnect, corrupt
//!   expert, speculation miss spike, verification failure)
//! - `TestGenerator::generate()` produces real Rust test source code
//!   encoding the failure condition — no more always-`None`
//!
//! ## Modules
//! - `mape` — MapeKLoop + SPC (Welford's algorithm) + VariationKind
//! - `knowledge` — AutonomicKnowledge + FailurePattern + AutonomicAction
//! - `remediation` — Remediation actions + RemediationResult + RemediationContext
//! - `test_generator` — antifragile test generation
//!
//! ## Dependencies
//! - `edgeforge-core` — ControlEvent
//! - `edgeforge-observability` — FunctionLogger for call logging
//! - `parking_lot` — Mutex/RwLock for thread-safe state
//! - `tokio::sync::mpsc` — event receiver
//! - `chrono` — timestamps
//!
//! ## MAPE-K Phases
//! | Phase | Implementation |
//! |---|---|
//! | Monitor | Drains a batch of ControlEvents from the event channel |
//! | Analyze | SPC (Welford's algorithm) distinguishes common-cause vs special-cause |
//! | Plan | Selects remediation based on the highest-severity failure pattern |
//! | Execute | Applies remediation against RemediationContext (mutates real state) |
//! | Knowledge | Streaming graph + CMMI metrics + failure pattern library + test generation |
//!
//! ## Antifragile Property
//! When a remediation succeeds, `TestGenerator::generate()` produces a
//! regression test encoding the failure condition. The test is added to the
//! CI suite so the same failure can never silently regress — the system
//! becomes stronger because it failed.

pub mod knowledge;
pub mod mape;
pub mod remediation;
pub mod test_generator;

pub use knowledge::{AutonomicAction, AutonomicKnowledge, FailureKind, FailurePattern};
pub use mape::{MapeKConfig, MapeKLoop, VariationKind};
pub use remediation::{
    Remediation, RemediationContext, RemediationContextSnapshot, RemediationKind,
    RemediationResult,
};
pub use test_generator::{GeneratedTest, TestGenerator, TestKind};
