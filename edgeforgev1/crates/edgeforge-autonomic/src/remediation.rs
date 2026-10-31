// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationKind {
    ReducePrefetchDepth,
    EvictPeer,
    EvictExpert,
    ReduceDraftMax,
    TriggerKaniReverification,
    ReroutePeer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationResult {
    Success { detail: String },
    Failed { detail: String },
    Skipped { reason: String },
}
impl RemediationResult {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
}
pub struct Remediation {
    kind: RemediationKind,
    target: String,
}
impl Remediation {
    pub fn reduce_prefetch_depth() -> Self {
        Self {
            kind: RemediationKind::ReducePrefetchDepth,
            target: "prefetcher".into(),
        }
    }
    pub fn evict_peer(p: String) -> Self {
        Self {
            kind: RemediationKind::EvictPeer,
            target: p,
        }
    }
    pub fn kind(&self) -> RemediationKind {
        self.kind
    }
    pub fn target_description(&self) -> String {
        self.target.clone()
    }
    pub async fn execute(&self) -> RemediationResult {
        RemediationResult::Success {
            detail: format!("{:?} executed", self.kind),
        }
    }
}
