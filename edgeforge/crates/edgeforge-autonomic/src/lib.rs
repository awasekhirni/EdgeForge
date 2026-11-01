// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-autonomic — L5.5 MAPE-K Autonomic Loop (ADR-029)
//!
//! ## Purpose
//! Monitor-Analyze-Plan-Execute over shared Knowledge. When a failure pattern
//! is resolved, the loop automatically generates a regression test — the
//! system becomes antifragile (stronger because it failed).
//!
//! ## Modules
//! - `mape` — MapeKLoop + SPC (Welford's algorithm) + VariationKind
//! - `knowledge` — AutonomicKnowledge + FailurePattern + AutonomicAction
//! - `remediation` — Remediation actions + RemediationResult
//! - `test_generator` — antifragile test generation
//!
//! ## Dependencies
//! - `edgeforge-core` — ControlEvent
//! - `edgeforge-graph` — StreamingGraph
//! - `parking_lot` — Mutex for stats
//! - `tokio::sync::mpsc` — event receiver
//!
//! ## MAPE-K Phases
//! | Phase | Implementation |
//! |---|---|
//! | Monitor | Subscribes to ControlEvent stream |
//! | Analyze | SPC distinguishes common-cause vs special-cause |
//! | Plan | Selects remediation based on failure pattern |
//! | Execute | Applies remediation idempotently |
//! | Knowledge | Streaming graph + CMMI metrics + failure pattern library |

pub mod knowledge;
pub mod mape;
pub mod remediation;
pub mod test_generator;
pub use knowledge::{AutonomicAction, AutonomicKnowledge, FailurePattern};
pub use mape::{MapeKConfig, MapeKLoop, VariationKind};
pub use remediation::{Remediation, RemediationKind, RemediationResult};
pub use test_generator::{GeneratedTest, TestGenerator, TestKind};
