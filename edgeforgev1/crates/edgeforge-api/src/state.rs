// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_orchestration::{Jobs, Models};
use std::sync::Arc;
#[derive(Clone)]
pub struct AppState {
    pub models: Arc<Models>,
    pub jobs: Arc<Jobs>,
    pub require_api_key: bool,
    pub rate_limit_per_minute: u32,
}
impl AppState {
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
