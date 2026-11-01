// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-core — Shared Leaf Crate
//!
//! ## Purpose
//! Every other crate in the workspace depends on this one. It provides the
//! foundational types (ModelId, ExpertId, LayerIdx, Token), the unified error
//! taxonomy (EdgeForgeError), the strongly-typed configuration system (Config),
//! the ControlEvent event bus schema (ADR-019), and physical-unit newtypes
//! (Bytes, TokPerSec, Celsius).
//!
//! ## What This File Does
//! Crate root — declares all public modules and re-exports the most commonly
//! used types so callers can write `use edgeforge_core::*`.
//!
//! ## Dependencies
//! - `thiserror` — EdgeForgeError derivation
//! - `serde` / `serde_json` — type serialization
//! - `figment` — typed config loading (TOML + env)
//! - `ulid` — ULID identifiers
//! - `tracing` — structured logging
//!
//! ## Architectural Invariants
//! - No internal dependencies (ADR-001: leaf-only).
//! - ControlEvent never carries prompt content (R-14, R-20).
//! - Config defaults: air-gapped, FP16, 2 TB NVMe minimum.

pub mod config;
pub mod control_event;
pub mod error;
pub mod types;
pub mod units;
pub use config::{
    ChatMessage, Config, NetworkMode, Precision, PrecisionPreset, ReleaseChannel, Role,
};
pub use control_event::{
    ControlEvent, ControlEventKind, CrowdTier, SpeculationHorizon, VerificationOutcome,
};
pub use error::{EdgeForgeError, Result};
pub use types::{
    ConversationId, ExpertId, ExpertSource, JobId, LayerIdx, MessageId, ModelId, NodeId, Prompt,
    Token,
};
pub use units::{bytes, celsius, tok_per_sec, Bytes, Celsius, TokPerSec};
