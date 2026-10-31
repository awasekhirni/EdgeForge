// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::EdgeForgeError;
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub read_paths: Vec<PathBuf>,
    pub write_paths: Vec<PathBuf>,
    pub network_egress: bool,
    pub allow_subprocess: bool,
}
impl SandboxPolicy {
    pub fn remote_peer(cache: &Path) -> Self {
        Self {
            read_paths: vec![cache.to_path_buf()],
            write_paths: vec![cache.to_path_buf()],
            network_egress: true,
            allow_subprocess: false,
        }
    }
}
pub struct Sandbox {
    policy: SandboxPolicy,
}
impl Sandbox {
    pub fn enforce(policy: SandboxPolicy) -> Result<Self, EdgeForgeError> {
        for w in &policy.write_paths {
            if !policy
                .read_paths
                .iter()
                .any(|r| r.starts_with(w) || w.starts_with(r))
            {
                return Err(EdgeForgeError::SandboxDenied(format!(
                    "write path {} not covered",
                    w.display()
                )));
            }
        }
        Ok(Self { policy })
    }
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
}
