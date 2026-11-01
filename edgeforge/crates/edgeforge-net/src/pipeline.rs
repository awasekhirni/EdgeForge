// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Pipeline Parallelism (F-NET-04)
//!
//! ## Purpose
//! Layer-stripe pipeline parallelism: node i computes layer i, forwards
//! activations to node i+1. Reduces per-token latency across multiple nodes.
//!
//! ## What This File Does
//! - `PipelineStage` — layer-to-node assignment
//! - `PipelinePlan` — full pipeline plan (all stages)
//! - `PipelinePlan::round_robin()` — build a round-robin plan
//! - `node_for_layer()` — which node computes a given layer
//! - `forward_activations()` — forward activations to the next stage
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, LayerIdx, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging

use edgeforge_core::{EdgeForgeError, LayerIdx, NodeId};
use edgeforge_observability::function_logger::FunctionGuard;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// A pipeline stage assignment: which node computes which layer.
#[derive(Clone, Debug)]
pub struct PipelineStage {
    /// The layer index this stage handles.
    pub layer: LayerIdx,
    /// The node assigned to compute this layer.
    pub node: NodeId,
}

/// A pipeline parallelism plan.
///
/// ## Purpose
/// Maps each layer to a node for pipeline-parallel inference. Node i computes
/// layer i and forwards activations to node i+1.
#[derive(Clone, Debug)]
pub struct PipelinePlan {
    /// All pipeline stages, ordered by layer.
    pub stages: Vec<PipelineStage>,
}

impl PipelinePlan {
    /// Build a plan that assigns layers round-robin to the given nodes.
    ///
    /// ## Parameters
    /// - `num_layers`: Total number of layers in the model.
    /// - `nodes`: Available nodes for pipeline parallelism.
    ///
    /// ## Returns
    /// A new `PipelinePlan` with round-robin layer assignment.
    pub fn round_robin(num_layers: u32, nodes: &[NodeId]) -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "round_robin");
        if nodes.is_empty() {
            return Self { stages: Vec::new() };
        }
        let stages = (0..num_layers)
            .map(|l| PipelineStage {
                layer: LayerIdx(l),
                node: nodes[(l as usize) % nodes.len()],
            })
            .collect();
        Self { stages }
    }

    /// Which node computes the given layer?
    ///
    /// ## Parameters
    /// - `layer`: The layer index.
    ///
    /// ## Returns
    /// `Some(NodeId)` if the layer is assigned, `None` otherwise.
    pub fn node_for_layer(&self, layer: LayerIdx) -> Option<NodeId> {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "node_for_layer");
        self.stages
            .iter()
            .find(|s| s.layer == layer)
            .map(|s| s.node)
    }

    /// Returns the number of stages in the plan.
    pub fn len(&self) -> usize {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "len");
        self.stages.len()
    }

    /// Returns true if the plan is empty.
    pub fn is_empty(&self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "is_empty");
        self.stages.is_empty()
    }

    /// Returns the node assigned to the layer after the given one.
    ///
    /// ## Parameters
    /// - `layer`: The current layer.
    ///
    /// ## Returns
    /// `Some(NodeId)` for the next layer's node, or `None` if this is the last layer.
    pub fn next_node(&self, layer: LayerIdx) -> Option<NodeId> {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "next_node");
        self.node_for_layer(layer.next())
    }
}

/// Forward activations to the next pipeline stage.
///
/// ## Purpose
/// Sends the activation tensor to the node that computes the next layer.
/// Uses the QUIC mesh transport's activation stream.
///
/// ## Parameters
/// - `activations`: The activation tensor bytes.
/// - `to`: The destination node ID.
///
/// ## Returns
/// `Ok(())` on success.
///
/// ## Errors
/// - `NotImplemented` — requires the quic-transport feature.
/// - `PeerUnreachable` if the destination node cannot be reached.
///
/// ## Dependencies
/// - `crate::quic_transport::MeshTransport::open_activation_stream` — for sending.
pub async fn forward_activations(_activations: &[u8], _to: NodeId) -> Result<(), EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "pipeline.rs",
        "forward_activations",
    );
    tracing::debug!("Forwarding activations to next pipeline stage");
    let err = EdgeForgeError::NotImplemented("forward_activations — enable quic-transport feature");
    guard.error(&err.to_string());
    Err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    #[test]
    fn test_round_robin_plan() {
        let n1 = NodeId(Ulid::new());
        let n2 = NodeId(Ulid::new());
        let plan = PipelinePlan::round_robin(6, &[n1, n2]);
        assert_eq!(plan.len(), 6);
        assert_eq!(plan.node_for_layer(LayerIdx(0)), Some(n1));
        assert_eq!(plan.node_for_layer(LayerIdx(1)), Some(n2));
        assert_eq!(plan.node_for_layer(LayerIdx(2)), Some(n1));
        assert_eq!(plan.node_for_layer(LayerIdx(3)), Some(n2));
    }

    #[test]
    fn test_round_robin_empty_nodes() {
        let plan = PipelinePlan::round_robin(10, &[]);
        assert!(plan.is_empty());
    }

    #[test]
    fn test_next_node() {
        let n1 = NodeId(Ulid::new());
        let n2 = NodeId(Ulid::new());
        let plan = PipelinePlan::round_robin(3, &[n1, n2]);
        assert_eq!(plan.next_node(LayerIdx(0)), Some(n2));
        assert_eq!(plan.next_node(LayerIdx(1)), Some(n1));
        assert_eq!(plan.next_node(LayerIdx(2)), None); // last layer
    }

    #[test]
    fn test_node_for_layer_missing() {
        let plan = PipelinePlan { stages: vec![] };
        assert!(plan.node_for_layer(LayerIdx(0)).is_none());
    }

    #[tokio::test]
    async fn test_forward_activations_not_implemented() {
        let result = forward_activations(&[1, 2, 3], NodeId(Ulid::new())).await;
        assert!(result.is_err());
    }
}
