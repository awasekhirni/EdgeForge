// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # OS-Native Sandbox Policy (F-SEC-03)
//!
//! ## Purpose
//! Defines sandbox policies that restrict what code can do (read/write paths,
//! network egress, subprocess spawning). Phase 2 ships the policy abstraction;
//! Phase 4+ wires OS enforcement (seatbelt/AppContainer/bubblewrap).
//!
//! ## What This File Does
//! - `SandboxPolicy` — struct with read_paths, write_paths, network_egress, allow_subprocess
//! - `SandboxPolicy::remote_peer(cache)` — policy for peer nodes (F-SEC-03)
//! - `Sandbox::enforce(policy)` — creates a Sandbox handle (no-op in Phase 2)
//!
//! ## Dependencies
//! - `edgeforge-core::EdgeForgeError` — for SandboxDenied errors
//!
//! ## Output
//! `Sandbox` handle — dropping it tears down the sandbox.
//!
//! ## Function: Sandbox::enforce
//! - **Purpose:** Create a sandbox from a policy
//! - **Parameters:** `SandboxPolicy` — the policy to enforce
//! - **Returns:** `Result<Sandbox, EdgeForgeError>` — Ok if policy is self-consistent
//! - **Errors:** SandboxDenied if a write path is not covered by any read path
//! - **Dependencies:** Validates write_paths ⊆ read_paths

use edgeforge_core::EdgeForgeError;
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, PartialEq, Eq)]
/// `SandboxPolicy` — see module documentation for purpose and usage.
pub struct SandboxPolicy {
    pub read_paths: Vec<PathBuf>,
    pub write_paths: Vec<PathBuf>,
    pub network_egress: bool,
    pub allow_subprocess: bool,
}
impl SandboxPolicy {
    /// Remote peer — see module documentation for details.
    pub fn remote_peer(cache: &Path) -> Self {
        Self {
            read_paths: vec![cache.to_path_buf()],
            write_paths: vec![cache.to_path_buf()],
            network_egress: true,
            allow_subprocess: false,
        }
    }
}
/// `Sandbox` — see module documentation for purpose and usage.
pub struct Sandbox {
    policy: SandboxPolicy,
}
impl Sandbox {
    /// Enforces the sandbox policy.
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
    /// Policy — see module documentation for details.
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
}
