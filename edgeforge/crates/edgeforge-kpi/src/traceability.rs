// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::qppo::ProcessAreaId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
pub type AdrId = String;
pub type ReqId = String;
pub type TestId = String;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ArchitecturalTrace {
    pub adr_ids: Vec<AdrId>,
    pub requirement_ids: Vec<ReqId>,
    pub verification_ids: Vec<TestId>,
    pub cmmi_process_areas: Vec<ProcessAreaId>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
}
