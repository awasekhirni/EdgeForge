// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-kpi — Architecture KPI Dashboard (ADR-030)
//!
//! ## Purpose
//! CMMI Level 5 quantitative process management. 8 frozen QPPOs with control
//! limits, bidirectional traceability matrix as CI gate, and carbon-aware
//! inference routing.
//!
//! ## Modules
//! - `qppo` — 8 frozen QPPOs + control limits + violation detection
//! - `traceability` — bidirectional traceability matrix (ADR ↔ req ↔ test ↔ code)
//! - `carbon` — CarbonAwareRouter + StaticCarbonTable (ADR-026)
//!
//! ## Dependencies
//! - `edgeforge-core` — types
//! - `async-trait`, `tokio` — for carbon intensity source
//! - `parking_lot` — thread-safe traceability matrix
//!
//! ## CMMI Compliance
//! | Area | Implementation |
//! |---|---|
//! | OPP | KPI Dashboard with 8 frozen QPPOs |
//! | CAR | MAPE-K loop (ADR-029) |
//! | DAR | Dual implementations (ADR-032) |
//! | TS | Kani proofs (ADR-025) |
//! | PI | Traceability matrix CI gate |

pub mod carbon;
pub mod qppo;
pub mod traceability;
pub use carbon::{CarbonAwareRouter, CarbonEstimate, CarbonIntensitySource};
pub use qppo::{ProcessAreaId, Qppo, QppoId, QppoSet, QppoStatus, QppoViolation};
pub use traceability::{AdrId, ArchitecturalTrace, ReqId, TestId, TraceabilityMatrix};
