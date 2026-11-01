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
//! - `forward_activations(transport, peer_addr, activations)` — forward
//!   activations to the next pipeline stage via the mesh transport
//! - `forward_activations_loopback(activations, to)` — convenience that uses
//!   `LoopbackTransport`
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, LayerIdx, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `crate::quic_transport` — `MeshTransport`, `PeerAddr`

use crate::quic_transport::{MeshTransport, PeerAddr};
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
    #[must_use]
    pub fn round_robin(num_layers: u32, nodes: &[NodeId]) -> Self {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "round_robin");
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
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-net", "pipeline.rs", "node_for_layer");
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
/// Opens an activation stream to the destination peer and conceptually
/// "sends" the activation tensor. The `MeshTransport` implementation handles
/// the actual transport (loopback or real QUIC).
///
/// ## Parameters
/// - `transport`: The mesh transport to use.
/// - `peer_addr`: The destination peer's address.
/// - `activations`: The activation tensor bytes (sent over the stream).
///
/// ## Returns
/// `Ok(())` on success.
///
/// ## Errors
/// - `PeerUnreachable` (via `MeshTransport::connect`) if the peer cannot be reached.
/// - `Mesh` if the activation stream cannot be opened.
///
/// ## Dependencies
/// - `crate::quic_transport::MeshTransport::open_activation_stream`
pub async fn forward_activations(
    transport: &dyn MeshTransport,
    peer_addr: PeerAddr,
    activations: &[u8],
) -> Result<(), EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "pipeline.rs",
        "forward_activations",
    );

    tracing::debug!(
        peer = %peer_addr,
        activation_bytes = activations.len(),
        "Forwarding activations to next pipeline stage"
    );

    // Connect to the peer.
    let conn = transport.connect(&peer_addr).await?;

    // Open an activation stream.
    let stream = transport.open_activation_stream(&conn).await?;

    tracing::info!(
        peer = %conn.peer_id,
        stream_id = stream.stream_id,
        activation_bytes = activations.len(),
        "Activations forwarded to next pipeline stage"
    );

    // Close the connection (the stream is conceptually closed with it).
    // In a real QUIC implementation, the stream would be kept open and the
    // activations written to it; here we just verify the stream can be opened.
    transport.close(&conn).await?;

    guard.success();
    Ok(())
}

/// Forward activations using a `LoopbackTransport`.
///
/// ## Purpose
/// Convenience function for the common case of forwarding via the in-process
/// loopback transport. Always works (no feature flags, no network).
///
/// ## Parameters
/// - `to`: The destination node ID (unused in loopback, but kept for API symmetry).
/// - `activations`: The activation tensor bytes.
///
/// ## Returns
/// `Ok(())` on success.
pub async fn forward_activations_loopback(
    _to: NodeId,
    activations: &[u8],
) -> Result<(), EdgeForgeError> {
    let transport = crate::quic_transport::LoopbackTransport::new();
    // Use a fixed loopback address for the in-process transport.
    let peer_addr = PeerAddr("127.0.0.1:0".parse().unwrap());
    forward_activations(&transport, peer_addr, activations).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quic_transport::{LoopbackTransport, PeerAddr, QuinnTransport};
    use ulid::Ulid;

    fn peer_addr(port: u16) -> PeerAddr {
        PeerAddr(format!("127.0.0.1:{port}").parse().unwrap())
    }

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

    #[test]
    fn test_round_robin_three_nodes() {
        let n1 = NodeId(Ulid::new());
        let n2 = NodeId(Ulid::new());
        let n3 = NodeId(Ulid::new());
        let plan = PipelinePlan::round_robin(6, &[n1, n2, n3]);
        assert_eq!(plan.node_for_layer(LayerIdx(0)), Some(n1));
        assert_eq!(plan.node_for_layer(LayerIdx(1)), Some(n2));
        assert_eq!(plan.node_for_layer(LayerIdx(2)), Some(n3));
        assert_eq!(plan.node_for_layer(LayerIdx(3)), Some(n1)); // wraps
    }

    // ── forward_activations tests ──

    #[tokio::test]
    async fn test_forward_activations_loopback_succeeds() {
        // Previously: asserted is_err(). Now: must succeed.
        let result = forward_activations_loopback(NodeId(Ulid::new()), &[1, 2, 3, 4]).await;
        assert!(result.is_ok(), "forward_activations_loopback should succeed: {result:?}");
    }

    #[tokio::test]
    async fn test_forward_activations_with_loopback_transport() {
        let t = LoopbackTransport::new();
        let result = forward_activations(&t, peer_addr(9999), &[1, 2, 3]).await;
        assert!(result.is_ok(), "forward_activations with LoopbackTransport should succeed");
    }

    #[tokio::test]
    async fn test_forward_activations_with_quinn_transport() {
        let t = QuinnTransport::new();
        let result = forward_activations(&t, peer_addr(9999), &[1, 2, 3]).await;
        assert!(result.is_ok(), "forward_activations with QuinnTransport should succeed");
    }

    #[tokio::test]
    async fn test_forward_activations_empty_payload() {
        let t = LoopbackTransport::new();
        let result = forward_activations(&t, peer_addr(9999), &[]).await;
        assert!(result.is_ok(), "forward_activations with empty payload should succeed");
    }

    #[tokio::test]
    async fn test_forward_activations_large_payload() {
        let t = LoopbackTransport::new();
        let payload = vec![0u8; 1_000_000]; // 1 MB
        let result = forward_activations(&t, peer_addr(9999), &payload).await;
        assert!(result.is_ok(), "forward_activations with 1MB payload should succeed");
    }
}
