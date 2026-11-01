// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Incentive Ledger (ADR-017)
//!
//! ## Purpose
//! Tier 2 credit accounting and Tier 3 token accounting. No on-chain dependency
//! by default (air-gapped principle, ADR-008).
//!
//! ## What This File Does
//! - `IncentivePolicy` — wraps LedgerKind
//! - `LedgerKind` — Credit { unit } or Token { initial_balance }
//! - `EntryKind` — Credit or Debit
//! - `LedgerEntry` — peer, tier, kind, amount, reason, related_job, created_at
//! - `IncentiveLedger` — thread-safe balances + log
//! - `post(entry)` — atomically update balance + append to log
//! - `balance(peer)` — current balance
//!
//! ## Dependencies
//! - `parking_lot::RwLock` — thread-safe balances + log
//! - `edgeforge_core::NodeId`
//! - `crate::CrowdTier`
//! - `serde` — for LedgerEntry serialization
//!
//! ## Output
//! `i64` — the peer's current balance.
//!
//! ## Function: post
//! - **Purpose:** Post a ledger entry and update the peer's balance
//! - **Parameters:** `LedgerEntry` — the entry to post
//! - **Side effects:** Updates `balances[peer] += amount`; appends to log
//! - **Dependencies:** `self.balances`, `self.log`

use crate::CrowdTier;
use edgeforge_core::NodeId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Debug)]
/// `IncentivePolicy` — see module documentation for purpose and usage.
pub struct IncentivePolicy {
    pub ledger: LedgerKind,
}
#[derive(Clone, Debug)]
/// `LedgerKind` enumeration — see module documentation for variants.
pub enum LedgerKind {
    Credit { unit: String },
    Token { initial_balance: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `EntryKind` enumeration — see module documentation for variants.
pub enum EntryKind {
    Credit,
    Debit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `LedgerEntry` — see module documentation for purpose and usage.
pub struct LedgerEntry {
    pub peer: NodeId,
    pub tier: CrowdTier,
    pub kind: EntryKind,
    pub amount: i64,
    pub reason: String,
    pub related_job: Option<String>,
    pub created_at_ms: i64,
}
/// `IncentiveLedger` — see module documentation for purpose and usage.
pub struct IncentiveLedger {
    balances: RwLock<HashMap<NodeId, i64>>,
    log: RwLock<Vec<LedgerEntry>>,
}
impl IncentiveLedger {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self {
            balances: RwLock::new(HashMap::new()),
            log: RwLock::new(Vec::new()),
        }
    }
    /// Posts a ledger entry and updates the balance.
    pub fn post(&self, entry: LedgerEntry) {
        let mut b = self.balances.write();
        *b.entry(entry.peer).or_insert(0) += entry.amount;
        self.log.write().push(entry);
    }
    /// Returns the current balance for a peer.
    pub fn balance(&self, peer: NodeId) -> i64 {
        self.balances.read().get(&peer).copied().unwrap_or(0)
    }
}
impl Default for IncentiveLedger {
    fn default() -> Self {
        Self::new()
    }
}
