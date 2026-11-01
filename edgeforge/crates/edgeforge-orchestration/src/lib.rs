// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
pub mod db;
pub mod event_bus;
pub mod jobs;
pub mod lifecycle;
pub mod presets;
pub mod scheduler;
pub use db::Models;
pub use jobs::{JobPriority, JobStatus, Jobs};
pub use lifecycle::ModelLifecycle;
pub use presets::PresetResolver;
pub use scheduler::Scheduler;

pub fn db_err(e: sqlx::Error) -> edgeforge_core::EdgeForgeError {
    edgeforge_core::EdgeForgeError::Database(e.to_string())
}
