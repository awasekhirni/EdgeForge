// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
pub enum QppoStatus {
    InControl,
    AboveUcl,
    BelowLcl,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
                    id: KaniCoverage,
                    name: "Kani coverage".into(),
                    unit: "%".into(),
                    upper_control_limit: 100.0,
                    lower_control_limit: 90.0,
                    target: 100.0,
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
