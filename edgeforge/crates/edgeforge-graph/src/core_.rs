// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Graph Core — In-Memory Graph Store (ADR-014)
//!
//! ## Purpose
//! The actual data store for the streaming graph. Uses `dashmap` for
//! concurrent node/edge access and `parking_lot` for the history writer lock.
//! Single-writer (ingest task), many-reader (router hook, prefetcher).
//!
//! ## What This File Does
//! - `GraphCore` — the graph store with peers, experts, history, health flag
//! - `PeerEstimate` — peer ID + RTT + fetch time + has_expert_resident
//! - `PeerNode` — full peer record (id, rtt, thermal, experts_resident, quality_score)
//! - `ExpertKey` — (LayerIdx, ExpertId) for expert residency tracking
//! - `ResidentLocation` — Ram, Ssd, or Peer(NodeId)
//! - `best_peer_for_expert()` — finds best peer by residency + RTT + quality
//! - `estimate_fetch_time()` — estimated fetch time in microseconds
//! - `record_heartbeat()` — update peer info from heartbeat
//! - `record_peer_lost()` — remove a peer
//! - `record_router_decision()` — add to sequence history (H3 prediction source)
//!
//! ## Dependencies
//! - `dashmap::DashMap` — concurrent hashmap for peers and experts
//! - `parking_lot::RwLock` — for sequence history
//! - `ahash::AHashMap` — fast hashmap for history
//! - `std::sync::atomic` — AtomicBool, AtomicU64 for health and counters
//!
//! ## Output
//! `Option<PeerEstimate>` — the best peer for a given (layer, expert), or None.
//!
//! ## Function: best_peer_for_expert
//! - **Purpose:** Find the best peer to fetch an expert from
//! - **Parameters:** `LayerIdx`, `ExpertId`, `Celsius` (thermal state)
//! - **Returns:** `Option<PeerEstimate>` — best peer or None if graph unhealthy
//! - **Selection criteria:** (1) peer has expert resident, (2) lowest RTT,
//!   (3) quality_score >= 0.3
//! - **Dependencies:** `self.healthy`, `self.peers`, `self.experts`

use ahash::AHashMap;
use dashmap::DashMap;
use edgeforge_core::{Celsius, ExpertId, LayerIdx, NodeId};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[derive(Clone, Copy, Debug)]
/// `PeerEstimate` — see module documentation for purpose and usage.
pub struct PeerEstimate {
    pub peer: NodeId,
    pub rtt_ms: u32,
    pub fetch_time_us: u64,
    pub has_expert_resident: bool,
}
/// `GraphCore` — see module documentation for purpose and usage.
pub struct GraphCore {
    pub(crate) peers: DashMap<NodeId, PeerNode>,
    pub(crate) experts: DashMap<ExpertKey, ResidentLocation>,
    pub(crate) history: RwLock<AHashMap<u64, Vec<(LayerIdx, ExpertId, i64)>>>,
    pub(crate) healthy: AtomicBool,
    pub(crate) events_ingested: AtomicU64,
    pub(crate) events_dropped: AtomicU64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// `ExpertKey` — see module documentation for purpose and usage.
pub struct ExpertKey {
    pub layer: LayerIdx,
    pub expert: ExpertId,
}
#[derive(Clone, Debug)]
/// `PeerNode` — see module documentation for purpose and usage.
pub struct PeerNode {
    pub id: NodeId,
    pub rtt_ms: u32,
    pub thermal_celsius: f32,
    pub experts_resident: u32,
    pub last_heartbeat_ms: i64,
    pub quality_score: f32,
}
#[derive(Clone, Debug)]
/// `ResidentLocation` enumeration — see module documentation for variants.
pub enum ResidentLocation {
    Ram,
    Ssd,
    Peer(NodeId),
}
impl GraphCore {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self {
            peers: DashMap::new(),
            experts: DashMap::new(),
            history: RwLock::new(AHashMap::new()),
            healthy: AtomicBool::new(true),
            events_ingested: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
        }
    }
    /// Best peer for expert — see module documentation for details.
    pub fn best_peer_for_expert(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
        _thermal: Celsius,
    ) -> Option<PeerEstimate> {
        if !self.healthy.load(Ordering::Relaxed) {
            return None;
        }
        let key = ExpertKey { layer, expert };
        let mut candidates: Vec<PeerEstimate> = self
            .peers
            .iter()
            .filter(|e| e.value().quality_score >= 0.3)
            .map(|e| {
                let p = e.value();
                PeerEstimate {
                    peer: p.id,
                    rtt_ms: p.rtt_ms,
                    fetch_time_us: u64::from(p.rtt_ms) * 1000,
                    has_expert_resident: false,
                }
            })
            .collect();
        if let Some(loc) = self.experts.get(&key) {
            if let ResidentLocation::Peer(pid) = *loc {
                for c in &mut candidates {
                    if c.peer == pid {
                        c.has_expert_resident = true;
                    }
                }
            }
        }
        candidates.sort_by(|a, b| {
            b.has_expert_resident
                .cmp(&a.has_expert_resident)
                .then(a.rtt_ms.cmp(&b.rtt_ms))
        });
        candidates.into_iter().next()
    }
    /// Estimate fetch time — see module documentation for details.
    pub fn estimate_fetch_time(&self, peer: NodeId, _expert: ExpertId) -> u64 {
        self.peers
            .get(&peer)
            .map(|p| u64::from(p.rtt_ms) * 1000)
            .unwrap_or(100_000)
    }
    /// Records a peer heartbeat.
    pub fn record_heartbeat(
        &self,
        peer: NodeId,
        rtt_ms: u32,
        thermal: f32,
        experts_resident: u32,
        now_ms: i64,
    ) {
        self.peers.insert(
            peer,
            PeerNode {
                id: peer,
                rtt_ms,
                thermal_celsius: thermal,
                experts_resident,
                last_heartbeat_ms: now_ms,
                quality_score: 0.5,
            },
        );
    }
    /// Records that a peer was lost.
    pub fn record_peer_lost(&self, peer: NodeId) {
        self.peers.remove(&peer);
    }
    /// Records a router decision in sequence history.
    pub fn record_router_decision(
        &self,
        seq: u64,
        layer: LayerIdx,
        experts: &[ExpertId],
        now_ms: i64,
    ) {
        let mut hist = self.history.write();
        let entry = hist.entry(seq).or_default();
        for &e in experts {
            entry.push((layer, e, now_ms));
        }
        if entry.len() > 4096 {
            let d = entry.len() - 4096;
            entry.drain(..d);
        }
    }
    /// Returns `true` if `healthy` is true.
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
    /// Sets the value of `healthy`.
    pub fn set_healthy(&self, h: bool) {
        self.healthy.store(h, Ordering::Relaxed);
    }
}
impl Default for GraphCore {
    fn default() -> Self {
        Self::new()
    }
}
