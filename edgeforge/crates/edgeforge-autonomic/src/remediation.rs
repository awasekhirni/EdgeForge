//! # Remediation Actions (ADR-029)
//!
//! ## Purpose
//! The set of actions the MAPE-K loop can take to remediate failures.
//! Each action is executed against a `RemediationContext` that owns real
//! mutable state (prefetch depth, evicted peers/experts, draft-max level),
//! so `execute()` actually changes system behavior — not hardcoded strings.
//!
//! ## What This File Does
//! - `RemediationKind` — ReducePrefetchDepth, EvictPeer, EvictExpert, etc.
//! - `RemediationResult` — Success, Failed, Skipped (with detail)
//! - `Remediation` — the action struct with kind + target
//! - `Remediation::execute()` — apply the remediation against a
//!   `RemediationContext`, mutating real state
//! - `RemediationContext` — thread-safe context owning the mutable state
//!   that remediations act on (prefetch depth, evicted peers/experts,
//!   draft-max, Kani re-verification queue, rerouted peers)
//!
//! ## Real State Mutation
//! Each remediation kind mutates real state in `RemediationContext`:
//! - `ReducePrefetchDepth` → decrements `prefetch_depth` (min 0); Skipped if already 0
//! - `EvictPeer` → inserts the peer into `evicted_peers`; Success if not already evicted
//! - `EvictExpert` → inserts the expert into `evicted_experts`; Success if not already evicted
//! - `ReduceDraftMax` → reduces `draft_max` from 16→8→4→2; Skipped if already at minimum (2)
//! - `TriggerKaniReverification` → pushes the target onto `kani_queue`; Success
//! - `ReroutePeer` → inserts the peer into `rerouted_peers`; Success
//!
//! ## Dependencies
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `parking_lot::Mutex` — thread-safe context state
//! - `serde` — for serialization
//!
//! ## Output
//! `RemediationResult` — Success, Failed, or Skipped with detail.
//!
//! ## Function Logging
//! `timestamp - edgeforge-autonomic - remediation.rs - function - STATUS`

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// The kind of remediation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationKind {
    /// Reduce the prefetcher's depth by 1 layer.
    ReducePrefetchDepth,
    /// Evict a peer from the mesh coordinator.
    EvictPeer,
    /// Evict an expert from the RAM cache.
    EvictExpert,
    /// Reduce the draft model's max tokens.
    ReduceDraftMax,
    /// Trigger a Kani re-verification of an unsafe block.
    TriggerKaniReverification,
    /// Reroute away from a specific peer.
    ReroutePeer,
}

impl RemediationKind {
    /// Returns the string name of the remediation kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReducePrefetchDepth => "reduce_prefetch_depth",
            Self::EvictPeer => "evict_peer",
            Self::EvictExpert => "evict_expert",
            Self::ReduceDraftMax => "reduce_draft_max",
            Self::TriggerKaniReverification => "trigger_kani_reverification",
            Self::ReroutePeer => "reroute_peer",
        }
    }
}

impl std::fmt::Display for RemediationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The result of a remediation action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationResult {
    /// The remediation was applied successfully.
    Success { detail: String },
    /// The remediation failed.
    Failed { detail: String },
    /// The remediation was skipped (e.g., already at minimum depth).
    Skipped { reason: String },
}

impl RemediationResult {
    /// Returns true if the result is Success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns true if the result is Skipped.
    #[must_use]
    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped { .. })
    }

    /// Returns true if the result is Failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns the detail string (or reason for Skipped).
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Success { detail } => detail,
            Self::Failed { detail } => detail,
            Self::Skipped { reason } => reason,
        }
    }
}

/// Thread-safe context that owns the mutable state remediations act on.
///
/// ## Purpose
/// Each `Remediation::execute()` call mutates this context. The context
/// tracks:
/// - `prefetch_depth` — the current prefetch depth (reduced by ReducePrefetchDepth)
/// - `evicted_peers` — peers evicted from the mesh (by EvictPeer)
/// - `evicted_experts` — experts evicted from the RAM cache (by EvictExpert)
/// - `draft_max` — the current draft-max (reduced by ReduceDraftMax)
/// - `kani_queue` — targets pending Kani re-verification (by TriggerKaniReverification)
/// - `rerouted_peers` — peers marked for rerouting (by ReroutePeer)
///
/// ## Thread Safety
/// All state is behind a `parking_lot::Mutex`. Safe to share across
/// threads via `Arc<RemediationContext>`.
#[derive(Debug)]
pub struct RemediationContext {
    inner: Mutex<RemediationContextInner>,
}

#[derive(Debug, Clone)]
struct RemediationContextInner {
    /// Current prefetch depth (layers). Reduced by ReducePrefetchDepth (min 0).
    prefetch_depth: u32,
    /// Peers evicted from the mesh coordinator (by peer ID string).
    evicted_peers: HashSet<String>,
    /// Experts evicted from the RAM cache (by "layer_N_expert_M" string).
    evicted_experts: HashSet<String>,
    /// Current draft-max token count. Reduced by ReduceDraftMax (16→8→4→2).
    draft_max: u32,
    /// Targets pending Kani re-verification.
    kani_queue: Vec<String>,
    /// Peers marked for rerouting.
    rerouted_peers: HashSet<String>,
}

impl Default for RemediationContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RemediationContext {
    /// Creates a new context with defaults:
    /// - `prefetch_depth = 4`
    /// - `draft_max = 16`
    /// - all sets empty
    #[must_use]
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "RemediationContext::new",
        );
        Self {
            inner: Mutex::new(RemediationContextInner {
                prefetch_depth: 4,
                evicted_peers: HashSet::new(),
                evicted_experts: HashSet::new(),
                draft_max: 16,
                kani_queue: Vec::new(),
                rerouted_peers: HashSet::new(),
            }),
        }
    }

    /// Creates a context with a custom initial prefetch depth + draft_max.
    #[must_use]
    pub fn with_initial_state(prefetch_depth: u32, draft_max: u32) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "RemediationContext::with_initial_state",
        );
        Self {
            inner: Mutex::new(RemediationContextInner {
                prefetch_depth,
                evicted_peers: HashSet::new(),
                evicted_experts: HashSet::new(),
                draft_max,
                kani_queue: Vec::new(),
                rerouted_peers: HashSet::new(),
            }),
        }
    }

    /// Returns the current prefetch depth.
    #[must_use]
    pub fn prefetch_depth(&self) -> u32 {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "remediation.rs", "prefetch_depth");
        self.inner.lock().prefetch_depth
    }

    /// Returns the current draft-max.
    #[must_use]
    pub fn draft_max(&self) -> u32 {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "remediation.rs", "draft_max");
        self.inner.lock().draft_max
    }

    /// Returns true if the given peer has been evicted.
    #[must_use]
    pub fn is_peer_evicted(&self, peer: &str) -> bool {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "is_peer_evicted",
        );
        self.inner.lock().evicted_peers.contains(peer)
    }

    /// Returns true if the given expert has been evicted.
    #[must_use]
    pub fn is_expert_evicted(&self, expert: &str) -> bool {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "is_expert_evicted",
        );
        self.inner.lock().evicted_experts.contains(expert)
    }

    /// Returns true if the given peer has been marked for rerouting.
    #[must_use]
    pub fn is_peer_rerouted(&self, peer: &str) -> bool {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "is_peer_rerouted",
        );
        self.inner.lock().rerouted_peers.contains(peer)
    }

    /// Returns the number of targets pending Kani re-verification.
    #[must_use]
    pub fn kani_queue_len(&self) -> usize {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "kani_queue_len",
        );
        self.inner.lock().kani_queue.len()
    }

    /// Returns the number of evicted peers.
    #[must_use]
    pub fn evicted_peer_count(&self) -> usize {
        self.inner.lock().evicted_peers.len()
    }

    /// Returns the number of evicted experts.
    #[must_use]
    pub fn evicted_expert_count(&self) -> usize {
        self.inner.lock().evicted_experts.len()
    }

    /// Returns a snapshot of the current context state (for testing / logging).
    #[must_use]
    pub fn snapshot(&self) -> RemediationContextSnapshot {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "remediation.rs", "snapshot");
        let inner = self.inner.lock();
        RemediationContextSnapshot {
            prefetch_depth: inner.prefetch_depth,
            draft_max: inner.draft_max,
            evicted_peer_count: inner.evicted_peers.len(),
            evicted_expert_count: inner.evicted_experts.len(),
            kani_queue_len: inner.kani_queue.len(),
            rerouted_peer_count: inner.rerouted_peers.len(),
        }
    }
}

/// A snapshot of the remediation context state (for testing / logging).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RemediationContextSnapshot {
    /// Current prefetch depth.
    pub prefetch_depth: u32,
    /// Current draft-max.
    pub draft_max: u32,
    /// Number of evicted peers.
    pub evicted_peer_count: usize,
    /// Number of evicted experts.
    pub evicted_expert_count: usize,
    /// Number of targets pending Kani re-verification.
    pub kani_queue_len: usize,
    /// Number of rerouted peers.
    pub rerouted_peer_count: usize,
}

/// A remediation action to be executed.
#[derive(Clone, Debug)]
pub struct Remediation {
    kind: RemediationKind,
    target: String,
}

impl Remediation {
    /// Creates a ReducePrefetchDepth remediation.
    #[must_use]
    pub fn reduce_prefetch_depth() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "reduce_prefetch_depth",
        );
        Self {
            kind: RemediationKind::ReducePrefetchDepth,
            target: "prefetcher".into(),
        }
    }

    /// Creates an EvictPeer remediation.
    #[must_use]
    pub fn evict_peer(peer: impl Into<String>) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "evict_peer",
        );
        Self {
            kind: RemediationKind::EvictPeer,
            target: peer.into(),
        }
    }

    /// Creates an EvictExpert remediation.
    #[must_use]
    pub fn evict_expert(expert: impl Into<String>) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "evict_expert",
        );
        Self {
            kind: RemediationKind::EvictExpert,
            target: expert.into(),
        }
    }

    /// Creates a ReduceDraftMax remediation.
    #[must_use]
    pub fn reduce_draft_max() -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "reduce_draft_max",
        );
        Self {
            kind: RemediationKind::ReduceDraftMax,
            target: "speculation".into(),
        }
    }

    /// Creates a TriggerKaniReverification remediation.
    #[must_use]
    pub fn trigger_kani_reverification(target: impl Into<String>) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "trigger_kani_reverification",
        );
        Self {
            kind: RemediationKind::TriggerKaniReverification,
            target: target.into(),
        }
    }

    /// Creates a ReroutePeer remediation.
    #[must_use]
    pub fn reroute_peer(peer: impl Into<String>) -> Self {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "reroute_peer",
        );
        Self {
            kind: RemediationKind::ReroutePeer,
            target: peer.into(),
        }
    }

    /// Returns the remediation kind.
    #[must_use]
    pub fn kind(&self) -> RemediationKind {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "remediation.rs", "kind");
        self.kind
    }

    /// Returns the target description.
    #[must_use]
    pub fn target_description(&self) -> String {
        let _guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "remediation.rs",
            "target_description",
        );
        self.target.clone()
    }

    /// Executes the remediation action against a `RemediationContext`.
    ///
    /// ## Purpose
    /// Applies the remediation to the context, mutating real state. Each
    /// kind produces a detailed result based on the actual state change:
    /// - `ReducePrefetchDepth` → decrements `prefetch_depth` (min 0);
    ///   `Skipped` if already 0
    /// - `EvictPeer` → inserts the peer into `evicted_peers`; `Success` if
    ///   newly evicted, `Skipped` if already evicted
    /// - `EvictExpert` → inserts the expert into `evicted_experts`; `Success`
    ///   if newly evicted, `Skipped` if already evicted
    /// - `ReduceDraftMax` → reduces `draft_max` from 16→8→4→2; `Skipped` if
    ///   already at minimum (2)
    /// - `TriggerKaniReverification` → pushes the target onto `kani_queue`;
    ///   `Success`
    /// - `ReroutePeer` → inserts the peer into `rerouted_peers`; `Success`
    ///
    /// ## Parameters
    /// - `ctx`: The remediation context to mutate.
    ///
    /// ## Returns
    /// `RemediationResult` — Success, Failed, or Skipped with detail.
    pub async fn execute(&self, ctx: &RemediationContext) -> RemediationResult {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "remediation.rs", "execute");
        tracing::info!(kind = %self.kind, target = %self.target, "Executing remediation");

        let result = {
            let mut inner = ctx.inner.lock();
            match self.kind {
                RemediationKind::ReducePrefetchDepth => {
                    if inner.prefetch_depth == 0 {
                        RemediationResult::Skipped {
                            reason: "Prefetch depth already at minimum (0)".to_string(),
                        }
                    } else {
                        inner.prefetch_depth -= 1;
                        RemediationResult::Success {
                            detail: format!(
                                "Prefetch depth reduced to {} (was {})",
                                inner.prefetch_depth,
                                inner.prefetch_depth + 1
                            ),
                        }
                    }
                }
                RemediationKind::EvictPeer => {
                    if inner.evicted_peers.insert(self.target.clone()) {
                        RemediationResult::Success {
                            detail: format!(
                                "Peer {} evicted from mesh coordinator (total evicted: {})",
                                self.target,
                                inner.evicted_peers.len()
                            ),
                        }
                    } else {
                        RemediationResult::Skipped {
                            reason: format!("Peer {} already evicted", self.target),
                        }
                    }
                }
                RemediationKind::EvictExpert => {
                    if inner.evicted_experts.insert(self.target.clone()) {
                        RemediationResult::Success {
                            detail: format!(
                                "Expert {} evicted from RAM cache (total evicted: {})",
                                self.target,
                                inner.evicted_experts.len()
                            ),
                        }
                    } else {
                        RemediationResult::Skipped {
                            reason: format!("Expert {} already evicted", self.target),
                        }
                    }
                }
                RemediationKind::ReduceDraftMax => {
                    let old = inner.draft_max;
                    let new = match old {
                        16 => 8,
                        8 => 4,
                        4 => 2,
                        _ => 2, // already at or below minimum
                    };
                    if old <= 2 {
                        RemediationResult::Skipped {
                            reason: format!("Draft-max already at minimum ({old})"),
                        }
                    } else {
                        inner.draft_max = new;
                        RemediationResult::Success {
                            detail: format!("Draft-max reduced from {old} to {new}"),
                        }
                    }
                }
                RemediationKind::TriggerKaniReverification => {
                    inner.kani_queue.push(self.target.clone());
                    RemediationResult::Success {
                        detail: format!(
                            "Kani re-verification of {} scheduled (queue depth: {})",
                            self.target,
                            inner.kani_queue.len()
                        ),
                    }
                }
                RemediationKind::ReroutePeer => {
                    let was_new = inner.rerouted_peers.insert(self.target.clone());
                    RemediationResult::Success {
                        detail: format!(
                            "Rerouted away from peer {}{}",
                            self.target,
                            if was_new { " (newly rerouted)" } else { " (already rerouted)" }
                        ),
                    }
                }
            }
        };

        if result.is_success() {
            guard.success();
        } else {
            guard.error(result.detail());
        }
        result
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── RemediationKind tests ──

    #[test]
    fn test_remediation_kind_as_str() {
        assert_eq!(RemediationKind::ReducePrefetchDepth.as_str(), "reduce_prefetch_depth");
        assert_eq!(RemediationKind::EvictPeer.as_str(), "evict_peer");
        assert_eq!(RemediationKind::EvictExpert.as_str(), "evict_expert");
        assert_eq!(RemediationKind::ReduceDraftMax.as_str(), "reduce_draft_max");
        assert_eq!(RemediationKind::TriggerKaniReverification.as_str(), "trigger_kani_reverification");
        assert_eq!(RemediationKind::ReroutePeer.as_str(), "reroute_peer");
    }

    #[test]
    fn test_remediation_kind_display() {
        assert_eq!(format!("{}", RemediationKind::EvictPeer), "evict_peer");
    }

    // ── RemediationResult tests ──

    #[test]
    fn test_remediation_result_is_success() {
        assert!(RemediationResult::Success { detail: "ok".into() }.is_success());
        assert!(!RemediationResult::Failed { detail: "err".into() }.is_success());
        assert!(!RemediationResult::Skipped { reason: "skip".into() }.is_success());
    }

    #[test]
    fn test_remediation_result_is_skipped() {
        assert!(RemediationResult::Skipped { reason: "skip".into() }.is_skipped());
        assert!(!RemediationResult::Success { detail: "ok".into() }.is_skipped());
    }

    #[test]
    fn test_remediation_result_is_failed() {
        assert!(RemediationResult::Failed { detail: "err".into() }.is_failed());
        assert!(!RemediationResult::Success { detail: "ok".into() }.is_failed());
    }

    #[test]
    fn test_remediation_result_detail() {
        assert_eq!(RemediationResult::Success { detail: "ok".into() }.detail(), "ok");
        assert_eq!(RemediationResult::Failed { detail: "err".into() }.detail(), "err");
        assert_eq!(RemediationResult::Skipped { reason: "skip".into() }.detail(), "skip");
    }

    // ── RemediationContext tests ──

    #[test]
    fn test_context_default_state() {
        let ctx = RemediationContext::new();
        let snap = ctx.snapshot();
        assert_eq!(snap.prefetch_depth, 4);
        assert_eq!(snap.draft_max, 16);
        assert_eq!(snap.evicted_peer_count, 0);
        assert_eq!(snap.evicted_expert_count, 0);
        assert_eq!(snap.kani_queue_len, 0);
        assert_eq!(snap.rerouted_peer_count, 0);
    }

    #[test]
    fn test_context_with_initial_state() {
        let ctx = RemediationContext::with_initial_state(2, 8);
        assert_eq!(ctx.prefetch_depth(), 2);
        assert_eq!(ctx.draft_max(), 8);
    }

    #[test]
    fn test_context_accessors() {
        let ctx = RemediationContext::new();
        assert!(!ctx.is_peer_evicted("p1"));
        assert!(!ctx.is_expert_evicted("e1"));
        assert!(!ctx.is_peer_rerouted("p1"));
        assert_eq!(ctx.kani_queue_len(), 0);
        assert_eq!(ctx.evicted_peer_count(), 0);
        assert_eq!(ctx.evicted_expert_count(), 0);
    }

    // ── execute() tests — real state mutation ──

    #[tokio::test]
    async fn test_reduce_prefetch_depth_succeeds_and_mutates() {
        let ctx = RemediationContext::new();
        assert_eq!(ctx.prefetch_depth(), 4);

        let r = Remediation::reduce_prefetch_depth();
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("3"));
        assert_eq!(ctx.prefetch_depth(), 3); // ← real state mutation
    }

    #[tokio::test]
    async fn test_reduce_prefetch_depth_to_zero() {
        let ctx = RemediationContext::with_initial_state(1, 16);
        let r = Remediation::reduce_prefetch_depth();

        // First call: 1 → 0
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert_eq!(ctx.prefetch_depth(), 0);

        // Second call: already 0 → Skipped
        let result = r.execute(&ctx).await;
        assert!(result.is_skipped());
        assert!(result.detail().contains("minimum"));
        assert_eq!(ctx.prefetch_depth(), 0); // unchanged
    }

    #[tokio::test]
    async fn test_evict_peer_succeeds_and_mutates() {
        let ctx = RemediationContext::new();
        let r = Remediation::evict_peer("peer-123");
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("peer-123"));
        assert!(ctx.is_peer_evicted("peer-123")); // ← real state mutation
        assert_eq!(ctx.evicted_peer_count(), 1);
    }

    #[tokio::test]
    async fn test_evict_peer_skipped_if_already_evicted() {
        let ctx = RemediationContext::new();
        let r = Remediation::evict_peer("peer-123");

        // First eviction: Success
        let result = r.execute(&ctx).await;
        assert!(result.is_success());

        // Second eviction of the same peer: Skipped
        let result = r.execute(&ctx).await;
        assert!(result.is_skipped());
        assert!(result.detail().contains("already evicted"));
        assert_eq!(ctx.evicted_peer_count(), 1); // unchanged
    }

    #[tokio::test]
    async fn test_evict_expert_succeeds_and_mutates() {
        let ctx = RemediationContext::new();
        let r = Remediation::evict_expert("layer_3_expert_5");
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("layer_3_expert_5"));
        assert!(ctx.is_expert_evicted("layer_3_expert_5")); // ← real state mutation
        assert_eq!(ctx.evicted_expert_count(), 1);
    }

    #[tokio::test]
    async fn test_evict_expert_skipped_if_already_evicted() {
        let ctx = RemediationContext::new();
        let r = Remediation::evict_expert("layer_3_expert_5");

        let result = r.execute(&ctx).await;
        assert!(result.is_success());

        let result = r.execute(&ctx).await;
        assert!(result.is_skipped());
        assert!(result.detail().contains("already evicted"));
    }

    #[tokio::test]
    async fn test_reduce_draft_max_steps_through_levels() {
        let ctx = RemediationContext::new();
        assert_eq!(ctx.draft_max(), 16);

        // 16 → 8
        let r = Remediation::reduce_draft_max();
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("16 to 8"));
        assert_eq!(ctx.draft_max(), 8);

        // 8 → 4
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert_eq!(ctx.draft_max(), 4);

        // 4 → 2
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert_eq!(ctx.draft_max(), 2);

        // 2 → Skipped (minimum)
        let result = r.execute(&ctx).await;
        assert!(result.is_skipped());
        assert_eq!(ctx.draft_max(), 2); // unchanged
    }

    #[tokio::test]
    async fn test_trigger_kani_reverification_queues_target() {
        let ctx = RemediationContext::new();
        let r = Remediation::trigger_kani_reverification("unsafe_block_42");
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("unsafe_block_42"));
        assert_eq!(ctx.kani_queue_len(), 1); // ← real state mutation

        // Queue a second target
        let r2 = Remediation::trigger_kani_reverification("unsafe_block_99");
        let result = r2.execute(&ctx).await;
        assert!(result.is_success());
        assert_eq!(ctx.kani_queue_len(), 2);
    }

    #[tokio::test]
    async fn test_reroute_peer_succeeds_and_mutates() {
        let ctx = RemediationContext::new();
        let r = Remediation::reroute_peer("peer-456");
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("peer-456"));
        assert!(ctx.is_peer_rerouted("peer-456")); // ← real state mutation
    }

    #[tokio::test]
    async fn test_reroute_peer_idempotent() {
        let ctx = RemediationContext::new();
        let r = Remediation::reroute_peer("peer-456");

        // First call: newly rerouted
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("newly rerouted"));

        // Second call: already rerouted (still Success, but different detail)
        let result = r.execute(&ctx).await;
        assert!(result.is_success());
        assert!(result.detail().contains("already rerouted"));
    }

    // ── Constructor + accessor tests ──

    #[test]
    fn test_remediation_kind_accessor() {
        let r = Remediation::reduce_prefetch_depth();
        assert_eq!(r.kind(), RemediationKind::ReducePrefetchDepth);
    }

    #[test]
    fn test_remediation_target_description() {
        let r = Remediation::evict_peer("test-peer");
        assert_eq!(r.target_description(), "test-peer");
    }

    #[test]
    fn test_remediation_clone_debug() {
        let r = Remediation::evict_expert("layer_0_expert_1");
        let cloned = r.clone();
        assert_eq!(r.kind(), cloned.kind());
        assert_eq!(r.target_description(), cloned.target_description());
        let debug = format!("{r:?}");
        assert!(debug.contains("EvictExpert"));
    }
}
