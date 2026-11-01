// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-orchestration — L5 Orchestration
//!
//! ## Purpose
//! Owns the model lifecycle (load → warm → serve → unload), job scheduling
//! (priority lanes + batch queues), distributed coordination, and the
//! ControlEvent publisher.
//!
//! ## Modules
//! - `db` — ORM models (Models) + SQLx queries
//! - `jobs` — job queue (Jobs) with status state machine
//! - `lifecycle` — model lifecycle state machine
//! - `scheduler` — priority-lane scheduler
//! - `presets` — preset resolver (Quality/Balanced/Fast/MaxQuality)
//! - `event_bus` — ControlEvent publisher (broadcast channel)
//!
//! ## Dependencies
//! - `edgeforge-core` — types, errors, config
//! - `edgeforge-runtime` — InferenceEngine
//! - `edgeforge-storage` — storage types
//! - `sqlx` — SQLite ORM
//! - `ulid` — for test code
//!
//! ## Output
//! `Models`, `Jobs`, `ModelLifecycle`, `Scheduler`, `PresetResolver`, `EventBus`.
//!
//! ## Function: db_err
//! - **Purpose:** Convert sqlx::Error to EdgeForgeError::Database
//! - **Parameters:** `sqlx::Error`
//! - **Returns:** `EdgeForgeError::Database`
//! - **Note:** Uses a free function (not From impl) to avoid the orphan rule

pub mod benchmark;
pub mod db;
pub mod event_bus;
pub mod jobs;
pub mod lifecycle;
pub mod mesh_coord;
pub mod presets;
pub mod quality_eval;
pub mod scheduler;
pub use db::{ModelArch, Models, NewModelRow};
pub use jobs::{JobPriority, JobStatus, Jobs};
pub use lifecycle::ModelLifecycle;
pub use presets::PresetResolver;
pub use scheduler::Scheduler;

/// Db err — see module documentation for details.
pub fn db_err(e: sqlx::Error) -> edgeforge_core::EdgeForgeError {
    edgeforge_core::EdgeForgeError::Database(e.to_string())
}
