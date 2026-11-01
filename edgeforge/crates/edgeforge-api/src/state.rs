// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Shared Application State
//!
//! ## Purpose
//! Top-level state shared by all axum handlers. Cheap to clone (all Arc).
//!
//! ## What This File Does
//! - `AppState` — Models, Jobs, require_api_key, rate_limit_per_minute
//! - `new()` — constructor
//!
//! ## Dependencies
//! - `edgeforge-orchestration::{Models, Jobs}`
//! - `std::sync::Arc`
//!
//! ## Output
//! `AppState` — cloned per request by axum.

use edgeforge_orchestration::{Jobs, Models};
use std::sync::Arc;
#[derive(Clone)]
/// `AppState` — see module documentation for purpose and usage.
pub struct AppState {
    pub models: Arc<Models>,
    pub jobs: Arc<Jobs>,
    pub require_api_key: bool,
    pub rate_limit_per_minute: u32,
}
impl AppState {
    /// Creates a new instance with default configuration.
    pub fn new(
        models: Models,
        jobs: Jobs,
        require_api_key: bool,
        rate_limit_per_minute: u32,
    ) -> Self {
        Self {
            models: Arc::new(models),
            jobs: Arc::new(jobs),
            require_api_key,
            rate_limit_per_minute,
        }
    }
}
