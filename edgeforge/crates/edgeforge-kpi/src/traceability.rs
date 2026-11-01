// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Bidirectional Traceability Matrix (ADR-030, CMMI PI)
//!
//! ## Purpose
//! Every public API item carries a [trace(...)] attribute linking it to ADRs,
//! requirements, tests, and CMMI process areas. CI fails any PR that
//! introduces a public item without a trace.
//!
//! ## What This File Does
//! - `ArchitecturalTrace` — adr_ids, requirement_ids, verification_ids, cmmi_process_areas
//! - `TraceabilityMatrix` — HashMap<item_path, ArchitecturalTrace>
//! - `add(path, trace)` — add a traceability entry
//! - `to_json()` — export as JSON for the KPI Dashboard
//!
//! ## Dependencies
//! - `crate::qppo::ProcessAreaId` — CMMI process area
//! - `serde` — serialization
//! - `std::collections::HashMap`
//!
//! ## Output
//! JSON string for the KPI Dashboard; `TraceabilityMatrix` for programmatic access.

use crate::qppo::ProcessAreaId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// Type alias for `AdrId`.
pub type AdrId = String;
pub type ReqId = String;
pub type TestId = String;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// `ArchitecturalTrace` — see module documentation for purpose and usage.
pub struct ArchitecturalTrace {
    pub adr_ids: Vec<AdrId>,
    pub requirement_ids: Vec<ReqId>,
    pub verification_ids: Vec<TestId>,
    pub cmmi_process_areas: Vec<ProcessAreaId>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// `TraceabilityMatrix` — see module documentation for purpose and usage.
pub struct TraceabilityMatrix {
    pub items: HashMap<String, ArchitecturalTrace>,
}
impl TraceabilityMatrix {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, path: &str, t: ArchitecturalTrace) {
        self.items.insert(path.into(), t);
    }
    /// Returns all item paths linked to the given ADR.
    pub fn items_for_adr(&self, adr: &str) -> Vec<&String> {
        self.items
            .iter()
            .filter(|(_, t)| t.adr_ids.iter().any(|a| a == adr))
            .map(|(k, _)| k)
            .collect()
    }
    /// Returns all tests linked to the given requirement.
    pub fn tests_for_requirement(&self, req: &str) -> Vec<&TestId> {
        self.items
            .values()
            .filter(|t| t.requirement_ids.iter().any(|r| r == req))
            .flat_map(|t| t.verification_ids.iter())
            .collect()
    }
    /// Export as JSON for the KPI Dashboard.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}
