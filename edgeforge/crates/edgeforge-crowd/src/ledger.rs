// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::CrowdTier;
use edgeforge_core::NodeId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Debug)]
pub struct IncentivePolicy {
    pub ledger: LedgerKind,
}
#[derive(Clone, Debug)]
pub enum LedgerKind {
    Credit { unit: String },
    Token { initial_balance: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Credit,
    Debit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub peer: NodeId,
    pub tier: CrowdTier,
    pub kind: EntryKind,
    pub amount: i64,
    pub reason: String,
    pub related_job: Option<String>,
    pub created_at_ms: i64,
}
pub struct IncentiveLedger {
    balances: RwLock<HashMap<NodeId, i64>>,
    log: RwLock<Vec<LedgerEntry>>,
}
impl IncentiveLedger {
    pub fn new() -> Self {
        Self {
            balances: RwLock::new(HashMap::new()),
            log: RwLock::new(Vec::new()),
        }
    }
    pub fn post(&self, entry: LedgerEntry) {
        let mut b = self.balances.write();
        *b.entry(entry.peer).or_insert(0) += entry.amount;
        self.log.write().push(entry);
    }
    pub fn balance(&self, peer: NodeId) -> i64 {
        self.balances.read().get(&peer).copied().unwrap_or(0)
    }
}
impl Default for IncentiveLedger {
    fn default() -> Self {
        Self::new()
    }
}
