// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use ahash::AHashMap;
use dashmap::DashMap;
use edgeforge_core::{Celsius, ExpertId, LayerIdx, NodeId};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[derive(Clone, Copy, Debug)]
pub struct PeerEstimate {
    pub peer: NodeId,
    pub rtt_ms: u32,
    pub fetch_time_us: u64,
    pub has_expert_resident: bool,
}
pub struct GraphCore {
    pub(crate) peers: DashMap<NodeId, PeerNode>,
    pub(crate) experts: DashMap<ExpertKey, ResidentLocation>,
    pub(crate) history: RwLock<AHashMap<u64, Vec<(LayerIdx, ExpertId, i64)>>>,
    pub(crate) healthy: AtomicBool,
    pub(crate) events_ingested: AtomicU64,
    pub(crate) events_dropped: AtomicU64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpertKey {
    pub layer: LayerIdx,
    pub expert: ExpertId,
}
#[derive(Clone, Debug)]
pub struct PeerNode {
    pub id: NodeId,
    pub rtt_ms: u32,
    pub thermal_celsius: f32,
    pub experts_resident: u32,
    pub last_heartbeat_ms: i64,
    pub quality_score: f32,
}
#[derive(Clone, Debug)]
pub enum ResidentLocation {
    Ram,
    Ssd,
    Peer(NodeId),
}
impl GraphCore {
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
    pub fn estimate_fetch_time(&self, peer: NodeId, _expert: ExpertId) -> u64 {
        self.peers
            .get(&peer)
            .map(|p| u64::from(p.rtt_ms) * 1000)
            .unwrap_or(100_000)
    }
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
    pub fn record_peer_lost(&self, peer: NodeId) {
        self.peers.remove(&peer);
    }
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
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
    pub fn set_healthy(&self, h: bool) {
        self.healthy.store(h, Ordering::Relaxed);
    }
}
impl Default for GraphCore {
    fn default() -> Self {
        Self::new()
    }
}
