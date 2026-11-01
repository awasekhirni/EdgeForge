//! # Distributed Mesh Coordinator (F-NET-*)
//!
//! ## Purpose
//! Tracks which node owns which expert, routes expert fetches to the
//! owning node, and handles node loss by reassigning experts.
//!
//! ## What This File Does
//! - `MeshCoordinator` — tracks expert-to-node ownership mappings
//! - `assign_experts()` — register that a node owns a set of experts
//! - `owner_of()` — query which node owns an expert
//! - `handle_node_loss()` — reassign experts when a node is lost
//! - `expert_count()` — number of tracked experts
//! - `node_count()` — number of tracked nodes
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, ExpertId, LayerIdx, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `parking_lot` — thread-safe interior mutability
//!
//! ## Output
//! `Option<NodeId>` from owner_of() — the owning node, or None.
//! `Vec<(LayerIdx, ExpertId)>` from handle_node_loss() — reassigned experts.
//!
//! ## Function Logging
//! `timestamp - edgeforge-orchestration - mesh_coord.rs - function - STATUS`

use edgeforge_core::{ExpertId, LayerIdx, NodeId};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The mesh coordinator. Tracks which node owns which expert.
///
/// ## Purpose
/// Maintains a mapping from (layer, expert) pairs to the owning NodeId.
/// When a node is lost, all its experts are returned for reassignment.
pub struct MeshCoordinator {
    /// Map: (layer, expert) → owning node.
    expert_owners: Mutex<HashMap<(LayerIdx, ExpertId), NodeId>>,
    /// Map: node → set of owned experts.
    node_experts: Mutex<HashMap<NodeId, HashSet<(LayerIdx, ExpertId)>>>,
}

impl MeshCoordinator {
    /// Creates a new empty mesh coordinator.
    ///
    /// ## Returns
    /// A new `MeshCoordinator` instance.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - MeshCoordinator::new - STATUS`
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "MeshCoordinator::new",
        );
        Self {
            expert_owners: Mutex::new(HashMap::new()),
            node_experts: Mutex::new(HashMap::new()),
        }
    }

    /// Register that a node owns a set of experts.
    ///
    /// ## Purpose
    /// Assigns ownership of the given experts to the specified node.
    /// If an expert was previously owned by another node, ownership is transferred.
    ///
    /// ## Parameters
    /// - `node`: The node that owns the experts.
    /// - `experts`: List of (layer, expert) pairs the node owns.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - assign_experts - STATUS`
    pub fn assign_experts(&self, node: NodeId, experts: &[(LayerIdx, ExpertId)]) {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "assign_experts",
        );
        let mut owners = self.expert_owners.lock();
        let mut node_map = self.node_experts.lock();

        for &(layer, expert) in experts {
            // Remove from previous owner if any.
            if let Some(&prev_owner) = owners.get(&(layer, expert)) {
                if let Some(prev_set) = node_map.get_mut(&prev_owner) {
                    prev_set.remove(&(layer, expert));
                }
            }
            // Assign to new owner.
            owners.insert((layer, expert), node);
            node_map
                .entry(node)
                .or_insert_with(HashSet::new)
                .insert((layer, expert));
        }

        tracing::info!(
            node = %node,
            count = experts.len(),
            "Assigned experts to node"
        );
    }

    /// Query which node owns a specific expert.
    ///
    /// ## Parameters
    /// - `layer`: The layer index.
    /// - `expert`: The expert ID.
    ///
    /// ## Returns
    /// `Option<NodeId>` — the owning node, or None if not tracked.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - owner_of - STATUS`
    pub fn owner_of(&self, layer: LayerIdx, expert: ExpertId) -> Option<NodeId> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "owner_of",
        );
        self.expert_owners.lock().get(&(layer, expert)).copied()
    }

    /// Handle node loss: return all experts that were owned by the lost node.
    ///
    /// ## Purpose
    /// When a node is lost (heartbeat timeout, crash, etc.), its experts
    /// need to be reassigned to other nodes or re-paged from local SSD.
    /// This function returns the list of orphaned experts and removes
    /// the node from the coordinator.
    ///
    /// ## Parameters
    /// - `node`: The node that was lost.
    ///
    /// ## Returns
    /// `Vec<(LayerIdx, ExpertId)>` — the experts that were owned by the lost node.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - handle_node_loss - STATUS`
    pub fn handle_node_loss(&self, node: NodeId) -> Vec<(LayerIdx, ExpertId)> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "handle_node_loss",
        );
        let mut owners = self.expert_owners.lock();
        let mut node_map = self.node_experts.lock();

        let lost: Vec<_> = node_map
            .remove(&node)
            .unwrap_or_default()
            .into_iter()
            .collect();

        for &key in &lost {
            owners.remove(&key);
        }

        tracing::warn!(node = %node, lost_experts = lost.len(), "Node lost — experts orphaned");
        lost
    }

    /// Returns the number of tracked experts.
    ///
    /// ## Returns
    /// `usize` — the number of tracked (layer, expert) pairs.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - expert_count - STATUS`
    pub fn expert_count(&self) -> usize {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "expert_count",
        );
        self.expert_owners.lock().len()
    }

    /// Returns the number of tracked nodes.
    ///
    /// ## Returns
    /// `usize` — the number of nodes with at least one expert.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - node_count - STATUS`
    pub fn node_count(&self) -> usize {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "node_count",
        );
        self.node_experts.lock().len()
    }

    /// Returns all experts owned by a specific node.
    ///
    /// ## Parameters
    /// - `node`: The node to query.
    ///
    /// ## Returns
    /// `Vec<(LayerIdx, ExpertId)>` — the experts owned by the node.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-orchestration - mesh_coord.rs - experts_for_node - STATUS`
    pub fn experts_for_node(&self, node: NodeId) -> Vec<(LayerIdx, ExpertId)> {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-orchestration",
            "mesh_coord.rs",
            "experts_for_node",
        );
        self.node_experts
            .lock()
            .get(&node)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }
}

impl Default for MeshCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn make_node() -> NodeId {
        NodeId(Ulid::new())
    }

    #[test]
    fn test_assign_and_query() {
        let mc = MeshCoordinator::new();
        let n1 = make_node();
        mc.assign_experts(n1, &[(LayerIdx(0), ExpertId(1)), (LayerIdx(0), ExpertId(2))]);
        assert_eq!(mc.owner_of(LayerIdx(0), ExpertId(1)), Some(n1));
        assert_eq!(mc.owner_of(LayerIdx(0), ExpertId(2)), Some(n1));
        assert_eq!(mc.owner_of(LayerIdx(1), ExpertId(1)), None);
        assert_eq!(mc.expert_count(), 2);
        assert_eq!(mc.node_count(), 1);
    }

    #[test]
    fn test_transfer_ownership() {
        let mc = MeshCoordinator::new();
        let n1 = make_node();
        let n2 = make_node();
        mc.assign_experts(n1, &[(LayerIdx(0), ExpertId(1))]);
        mc.assign_experts(n2, &[(LayerIdx(0), ExpertId(1))]); // transfer
        assert_eq!(mc.owner_of(LayerIdx(0), ExpertId(1)), Some(n2));
        assert_eq!(mc.experts_for_node(n1).len(), 0);
        assert_eq!(mc.experts_for_node(n2).len(), 1);
    }

    #[test]
    fn test_node_loss_returns_orphans() {
        let mc = MeshCoordinator::new();
        let n1 = make_node();
        mc.assign_experts(n1, &[(LayerIdx(0), ExpertId(1)), (LayerIdx(1), ExpertId(2))]);
        let lost = mc.handle_node_loss(n1);
        assert_eq!(lost.len(), 2);
        assert_eq!(mc.owner_of(LayerIdx(0), ExpertId(1)), None);
        assert_eq!(mc.expert_count(), 0);
        assert_eq!(mc.node_count(), 0);
    }

    #[test]
    fn test_node_loss_unknown_node() {
        let mc = MeshCoordinator::new();
        let n = make_node();
        let lost = mc.handle_node_loss(n);
        assert!(lost.is_empty());
    }

    #[test]
    fn test_experts_for_node_empty() {
        let mc = MeshCoordinator::new();
        let n = make_node();
        assert!(mc.experts_for_node(n).is_empty());
    }
}
