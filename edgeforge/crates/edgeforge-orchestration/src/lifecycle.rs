// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::{EdgeForgeError, ModelId};
use std::collections::HashMap;
use std::sync::Mutex;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleState {
    Unloaded,
    Loading,
    Warming,
    Serving,
    Unloading,
    Failed,
}
pub struct ModelLifecycle {
    states: Mutex<HashMap<ModelId, LifecycleState>>,
}
impl ModelLifecycle {
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
        }
    }
    pub fn state(&self, id: ModelId) -> LifecycleState {
        self.states
            .lock()
            .ok()
            .and_then(|g| g.get(&id).copied())
            .unwrap_or(LifecycleState::Unloaded)
    }
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
