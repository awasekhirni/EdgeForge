// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Quantitative Process Performance Objectives (ADR-030)
//!
//! ## Purpose
//! 8 frozen QPPOs with upper/lower control limits. When a metric crosses a
//! control limit, a CAR workflow fires, generating a draft ADR.
//!
//! ## What This File Does
//! - `QppoId` — 8 frozen metrics (MttrDeploys, RegressionEscapeRate, DefectDensity,
//!   KaniCoverage, BuildTimeP95, TestFlakeRate, UnsafeLocPerCrate, DependencyFreshness)
//! - `ProcessAreaId` — CMMI process areas (CAR, DAR, OPP, TS, PI, VV, RSK, CM)
//! - `Qppo` — id, name, unit, UCL, LCL, target, cmmi_area
//! - `QppoStatus` — InControl, AboveUcl, BelowLcl
//! - `QppoViolation` — qppo_id, measured_value, status, draft_adr_id
//! - `QppoSet` — the frozen set of 8 QPPOs
//! - `evaluate(id, value, now_ms)` — classify a measurement, return violation if out of control
//!
//! ## Dependencies
//! - `serde` — serialization
//!
//! ## Output
//! `Option<QppoViolation>` — Some if the metric is out of control, None if in control.
//!
//! ## The 8 Frozen QPPOs
//! | # | Metric | UCL | LCL | CMMI Area |
//! |---|---|---|---|---|
//! | 1 | MTTR deploys | 30 min | 0 | CAR |
//! | 2 | Regression escape rate | 5% | 0 | CAR |
//! | 3 | Defect density | 5/kLOC | 0 | DAR |
//! | 4 | Kani coverage | 100% | 90% | TS |
//! | 5 | Build time p95 | 30 min | 0 | OPP |
//! | 6 | Test flake rate | 2% | 0 | OPP |
//! | 7 | unsafe LOC per crate | 500 | 0 | TS |
//! | 8 | Dependency freshness | 90 days | 0 | TS |

use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `QppoId` enumeration — see module documentation for variants.
pub enum QppoId {
    MttrDeploys,
    RegressionEscapeRate,
    DefectDensity,
    KaniCoverage,
    BuildTimeP95,
    TestFlakeRate,
    UnsafeLocPerCrate,
    DependencyFreshness,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `ProcessAreaId` enumeration — see module documentation for variants.
pub enum ProcessAreaId {
    Car,
    Dar,
    Opp,
    Ts,
    Pi,
    Vv,
    Rsk,
    Cm,
}
impl QppoId {
    pub const fn cmmi_area(self) -> ProcessAreaId {
        match self {
            Self::MttrDeploys | Self::RegressionEscapeRate => ProcessAreaId::Car,
            Self::DefectDensity => ProcessAreaId::Dar,
            Self::KaniCoverage | Self::UnsafeLocPerCrate | Self::DependencyFreshness => {
                ProcessAreaId::Ts
            }
            Self::BuildTimeP95 | Self::TestFlakeRate => ProcessAreaId::Opp,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `Qppo` — see module documentation for purpose and usage.
pub struct Qppo {
    pub id: QppoId,
    pub name: String,
    pub unit: String,
    pub upper_control_limit: f64,
    pub lower_control_limit: f64,
    pub target: f64,
    pub cmmi_area: ProcessAreaId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `QppoStatus` enumeration — see module documentation for variants.
pub enum QppoStatus {
    InControl,
    AboveUcl,
    BelowLcl,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `QppoViolation` — see module documentation for purpose and usage.
pub struct QppoViolation {
    pub qppo_id: QppoId,
    pub measured_value: f64,
    pub status: QppoStatus,
    pub timestamp_ms: i64,
    pub draft_adr_id: Option<String>,
}
impl Qppo {
    pub fn classify(&self, v: f64) -> QppoStatus {
        if v > self.upper_control_limit {
            QppoStatus::AboveUcl
        } else if v < self.lower_control_limit {
            QppoStatus::BelowLcl
        } else {
            QppoStatus::InControl
        }
    }
}
#[derive(Clone, Debug)]
/// `QppoSet` — see module documentation for purpose and usage.
pub struct QppoSet {
    pub qppos: Vec<Qppo>,
}
impl Default for QppoSet {
    fn default() -> Self {
        use ProcessAreaId::*;
        use QppoId::*;
        Self {
            qppos: vec![
                Qppo {
                    id: MttrDeploys,
                    name: "MTTR deploys".into(),
                    unit: "min".into(),
                    upper_control_limit: 30.0,
                    lower_control_limit: 0.0,
                    target: 5.0,
                    cmmi_area: Car,
                },
                Qppo {
                    id: RegressionEscapeRate,
                    name: "Regression escape rate".into(),
                    unit: "%".into(),
                    upper_control_limit: 5.0,
                    lower_control_limit: 0.0,
                    target: 0.0,
                    cmmi_area: Car,
                },
                Qppo {
                    id: DefectDensity,
                    name: "Defect density".into(),
                    unit: "/kLOC".into(),
                    upper_control_limit: 5.0,
                    lower_control_limit: 0.0,
                    target: 1.0,
                    cmmi_area: Dar,
                },
                Qppo {
                    id: KaniCoverage,
                    name: "Kani coverage".into(),
                    unit: "%".into(),
                    upper_control_limit: 100.0,
                    lower_control_limit: 90.0,
                    target: 100.0,
                    cmmi_area: Ts,
                },
                Qppo {
                    id: BuildTimeP95,
                    name: "Build time p95".into(),
                    unit: "min".into(),
                    upper_control_limit: 30.0,
                    lower_control_limit: 0.0,
                    target: 15.0,
                    cmmi_area: Opp,
                },
                Qppo {
                    id: TestFlakeRate,
                    name: "Test flake rate".into(),
                    unit: "%".into(),
                    upper_control_limit: 2.0,
                    lower_control_limit: 0.0,
                    target: 0.0,
                    cmmi_area: Opp,
                },
                Qppo {
                    id: UnsafeLocPerCrate,
                    name: "unsafe LOC per crate".into(),
                    unit: "LOC".into(),
                    upper_control_limit: 500.0,
                    lower_control_limit: 0.0,
                    target: 0.0,
                    cmmi_area: Ts,
                },
                Qppo {
                    id: DependencyFreshness,
                    name: "Dependency freshness".into(),
                    unit: "days".into(),
                    upper_control_limit: 90.0,
                    lower_control_limit: 0.0,
                    target: 30.0,
                    cmmi_area: Ts,
                },
            ],
        }
    }
}
impl QppoSet {
    pub fn evaluate(&self, id: QppoId, v: f64, now: i64) -> Option<QppoViolation> {
        let q = self.qppos.iter().find(|q| q.id == id)?;
        let s = q.classify(v);
        if s == QppoStatus::InControl {
            None
        } else {
            Some(QppoViolation {
                qppo_id: id,
                measured_value: v,
                status: s,
                timestamp_ms: now,
                draft_adr_id: None,
            })
        }
    }
}
