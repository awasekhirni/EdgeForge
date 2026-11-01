// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-graph — L3.5 Streaming Property Graph (ADR-014)
//!
//! ## Purpose
//! A derived, in-memory property graph that gives the router, scheduler, and
//! prefetcher real-time visibility into expert residency, peer bandwidth, and
//! thermal state. NOT a source of truth — falls back to deterministic SSD
//! behavior on failure.
//!
//! ## What This File Does
//! - `StreamingGraph` trait — best_peer_for_expert, estimate_fetch_time,
//!   predicted_expert_set, ingest, healthy
//! - `GraphError` — Dropped, Unhealthy
//! - Re-exports `GraphCore`, `GraphIngest`, `GraphQuery`
//!
//! ## Dependencies
//! - `edgeforge-core` — ControlEvent, ExpertId, LayerIdx, NodeId, Celsius
//! - `dashmap` — concurrent node/edge access
//! - `parking_lot` — ingest writer lock
//! - `crossbeam-queue` — bounded ingest channel
//!
//! ## Architectural Invariants
//! 1. Graph is derived only — never stores authoritative state.
//! 2. `healthy() == false` → callers MUST fall back to SSD.
//! 3. Graph MUST NOT depend on edgeforge-runtime or edgeforge-storage.
//! 4. Ingest is bounded; oldest events dropped under load (R-18).
//!
//! ## Query Contract
//! `best_peer_for_expert()` must complete in <10 µs p99 (ADR-014).
//! CI gate: `benches/query_latency.rs` asserts this contract.

pub mod core_;
pub mod ingest;
pub mod query;
use async_trait::async_trait;
pub use core_::{GraphCore, PeerEstimate};
use edgeforge_core::{Celsius, ControlEvent, ExpertId, LayerIdx, NodeId};
pub use ingest::{GraphIngest, GraphIngestConfig};
pub use query::{GraphQuery, GraphQueryError};
#[async_trait]
/// The `StreamingGraph` trait — see module documentation for the contract.
pub trait StreamingGraph: Send + Sync + 'static {
    fn best_peer_for_expert(
        &self,
        layer: LayerIdx,
        expert: ExpertId,
        thermal: Celsius,
    ) -> Option<PeerEstimate>;
    fn estimate_fetch_time(&self, peer: NodeId, expert: ExpertId) -> u64;
    fn predicted_expert_set(&self, sequence: u64, n: u32) -> Vec<(LayerIdx, ExpertId)>;
    fn ingest(&self, event: ControlEvent) -> Result<(), GraphError>;
    fn healthy(&self) -> bool;
}
#[derive(Debug, thiserror::Error)]
/// `GraphError` enumeration — see module documentation for variants.
pub enum GraphError {
    #[error("dropped")]
    Dropped,
    #[error("unhealthy")]
    Unhealthy,
}
