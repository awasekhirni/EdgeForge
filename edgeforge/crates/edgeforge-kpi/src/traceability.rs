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
pub type AdrId = String; pub type ReqId = String; pub type TestId = String;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// `ArchitecturalTrace` — see module documentation for purpose and usage.
pub struct ArchitecturalTrace { pub adr_ids: Vec<AdrId>, pub requirement_ids: Vec<ReqId>, pub verification_ids: Vec<TestId>, pub cmmi_process_areas: Vec<ProcessAreaId> }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// `TraceabilityMatrix` — see module documentation for purpose and usage.
pub struct TraceabilityMatrix { pub items: HashMap<String, ArchitecturalTrace> }
impl TraceabilityMatrix {
    pub fn new() -> Self { Self::default() }
    pub fn add(&mut self, path: &str, t: ArchitecturalTrace) { self.items.insert(path.into(), t); }
    /// Returns all item paths linked to the given ADR.
    pub fn items_for_adr(&self, adr: &str) -> Vec<&String> {
        self.items.iter().filter(|(_, t)| t.adr_ids.iter().any(|a| a == adr)).map(|(k, _)| k).collect()
    }
    /// Returns all tests linked to the given requirement.
    pub fn tests_for_requirement(&self, req: &str) -> Vec<&TestId> {
        self.items.values().filter(|t| t.requirement_ids.iter().any(|r| r == req)).flat_map(|t| t.verification_ids.iter()).collect()
    }
    /// Export as JSON for the KPI Dashboard.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qppo::ProcessAreaId;

    #[test]
    fn test_new_matrix_is_empty() {
        let m = TraceabilityMatrix::new();
        assert!(m.items.is_empty());
    }

    #[test]
    fn test_add_and_retrieve() {
        let mut m = TraceabilityMatrix::new();
        m.add("edgeforge_core::ModelId", ArchitecturalTrace {
            adr_ids: vec!["ADR-001".into()],
            requirement_ids: vec!["F-LOAD-01".into()],
            verification_ids: vec!["tests/core.rs".into()],
            cmmi_process_areas: vec![ProcessAreaId::Ts],
        });
        assert_eq!(m.items.len(), 1);
        assert!(m.items.contains_key("edgeforge_core::ModelId"));
    }

    #[test]
    fn test_items_for_adr() {
        let mut m = TraceabilityMatrix::new();
        m.add("item1", ArchitecturalTrace {
            adr_ids: vec!["ADR-001".into()],
            ..Default::default()
        });
        m.add("item2", ArchitecturalTrace {
            adr_ids: vec!["ADR-002".into()],
            ..Default::default()
        });
        m.add("item3", ArchitecturalTrace {
            adr_ids: vec!["ADR-001".into(), "ADR-002".into()],
            ..Default::default()
        });
        let items = m.items_for_adr("ADR-001");
        assert_eq!(items.len(), 2);
        let items2 = m.items_for_adr("ADR-002");
        assert_eq!(items2.len(), 2);
    }

    #[test]
    fn test_items_for_adr_no_match() {
        let mut m = TraceabilityMatrix::new();
        m.add("item1", ArchitecturalTrace::default());
        let items = m.items_for_adr("ADR-999");
        assert!(items.is_empty());
    }

    #[test]
    fn test_tests_for_requirement() {
        let mut m = TraceabilityMatrix::new();
        m.add("item1", ArchitecturalTrace {
            requirement_ids: vec!["F-LOAD-01".into()],
            verification_ids: vec!["tests/a.rs".into(), "tests/b.rs".into()],
            ..Default::default()
        });
        m.add("item2", ArchitecturalTrace {
            requirement_ids: vec!["F-LOAD-02".into()],
            verification_ids: vec!["tests/c.rs".into()],
            ..Default::default()
        });
        let tests = m.tests_for_requirement("F-LOAD-01");
        assert_eq!(tests.len(), 2);
    }

    #[test]
    fn test_tests_for_requirement_no_match() {
        let mut m = TraceabilityMatrix::new();
        m.add("item1", ArchitecturalTrace::default());
        let tests = m.tests_for_requirement("F-NONEXISTENT");
        assert!(tests.is_empty());
    }

    #[test]
    fn test_to_json_contains_items() {
        let mut m = TraceabilityMatrix::new();
        m.add("test::item", ArchitecturalTrace {
            adr_ids: vec!["ADR-001".into()],
            ..Default::default()
        });
        let json = m.to_json();
        assert!(json.contains("test::item"));
        assert!(json.contains("ADR-001"));
    }

    #[test]
    fn test_to_json_empty_matrix() {
        let m = TraceabilityMatrix::new();
        let json = m.to_json();
        assert!(json.contains("items"));
    }

    #[test]
    fn test_default_trace() {
        let t = ArchitecturalTrace::default();
        assert!(t.adr_ids.is_empty());
        assert!(t.requirement_ids.is_empty());
        assert!(t.verification_ids.is_empty());
        assert!(t.cmmi_process_areas.is_empty());
    }

    #[test]
    fn test_add_overwrites_existing() {
        let mut m = TraceabilityMatrix::new();
        m.add("item", ArchitecturalTrace {
            adr_ids: vec!["ADR-001".into()],
            ..Default::default()
        });
        m.add("item", ArchitecturalTrace {
            adr_ids: vec!["ADR-002".into()],
            ..Default::default()
        });
        assert_eq!(m.items.len(), 1);
        assert_eq!(m.items.get("item").unwrap().adr_ids, vec!["ADR-002"]);
    }
}
