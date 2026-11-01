// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Model Lifecycle State Machine
//!
//! ## Purpose
//! Tracks which models are loaded and in what state. Enforces the legal
//! transition graph: Unloaded → Loading → Warming → Serving → Unloading → Unloaded.
//!
//! ## What This File Does
//! - `LifecycleState` — Unloaded, Loading, Warming, Serving, Unloading, Failed
//! - `ModelLifecycle` — thread-safe state tracker (Mutex<HashMap>)
//! - `state(id)` — get the current state of a model
//! - `transition(id, to)` — transition to a new state (enforces legal graph)
//!
//! ## Dependencies
//! - `edgeforge-core::{EdgeForgeError, ModelId}`
//! - `std::sync::Mutex` — thread-safe state map
//!
//! ## Output
//! `Result<(), EdgeForgeError>` — Ok if transition is legal, InvalidJobState if not.
//!
//! ## Legal Transitions
//! ```text
//! Unloaded → Loading → Warming → Serving → Unloading → Unloaded
//!                 ↘ Failed ↗         ↘ Failed ↗
//! Failed → Unloaded (recovery)
//! ```
//!
//! ## Function: transition
//! - **Purpose:** Transition a model to a new lifecycle state
//! - **Parameters:** `ModelId`, `LifecycleState` (target)
//! - **Returns:** `Result<(), EdgeForgeError>` — Ok if legal, InvalidJobState if not
//! - **Errors:** InvalidJobState if the transition is not in the legal graph
//! - **Dependencies:** `is_legal(from, to)` — checks the transition graph

use edgeforge_core::{EdgeForgeError, ModelId};
use std::collections::HashMap;
use std::sync::Mutex;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// `LifecycleState` enumeration — see module documentation for variants.
pub enum LifecycleState {
    Unloaded,
    Loading,
    Warming,
    Serving,
    Unloading,
    Failed,
}
/// `ModelLifecycle` — see module documentation for purpose and usage.
pub struct ModelLifecycle {
    states: Mutex<HashMap<ModelId, LifecycleState>>,
}
impl ModelLifecycle {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
        }
    }
    /// Returns the current state.
    pub fn state(&self, id: ModelId) -> LifecycleState {
        self.states
            .lock()
            .ok()
            .and_then(|g| g.get(&id).copied())
            .unwrap_or(LifecycleState::Unloaded)
    }
    /// Transition — see module documentation for details.
    pub fn transition(&self, id: ModelId, to: LifecycleState) -> Result<(), EdgeForgeError> {
        let mut g = self
            .states
            .lock()
            .map_err(|_| EdgeForgeError::Engine("lock poisoned".into()))?;
        let from = g.get(&id).copied().unwrap_or(LifecycleState::Unloaded);
        if !is_legal(from, to) {
            return Err(EdgeForgeError::InvalidJobState {
                job: format!("model:{id}"),
                state: format!("{from:?}->{to:?}"),
            });
        }
        g.insert(id, to);
        Ok(())
    }
}
fn is_legal(from: LifecycleState, to: LifecycleState) -> bool {
    use LifecycleState::*;
    match (from, to) {
        (Unloaded, Loading) => true,
        (Loading, Warming | Failed | Unloaded) => true,
        (Warming, Serving | Failed | Unloading) => true,
        (Serving, Unloading | Failed) => true,
        (Unloading, Unloaded | Failed) => true,
        (Failed, Unloaded) => true,
        (a, b) if a == b => true,
        _ => false,
    }
}
impl Default for ModelLifecycle {
    fn default() -> Self {
        Self::new()
    }
}
