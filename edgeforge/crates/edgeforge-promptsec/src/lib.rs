// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-promptsec — L1.5 Prompt Security (ADRs 020–023)
//!
//! ## Purpose
//! Defends against prompt-injection attacks — the class of attack where an
//! adversary embeds instructions inside retrieved documents, tool outputs, or
//! other external content. The v2.0 network/sandbox layers do not cover this
//! threat; L1.5 closes the gap.
//!
//! ## Four Defenses
//! 1. **Dual-LLM Privilege Separation** (ADR-020) — privileged planner (sees
//!    prompt, no tools) + unprivileged worker (tools, no prompt)
//! 2. **Spotlighting** (ADR-021) — data marking + encapsulation + trimming on
//!    all untrusted external content
//! 3. **Intent Guardrails** (ADR-022) — two-stage classifier + rule engine
//! 4. **Quarantine Reader LLM** (ADR-023) — separate isolated LLM reads raw
//!    external content; main model sees only sanitized summaries
//!
//! ## Pipeline
//! User prompt → guardrails (pre-flight) → planner → quarantine reader →
//! spotlighting → worker → guardrails (post-decode) → execute tool
//!
//! ## Dependencies
//! - `edgeforge-core` — Prompt, Token, EdgeForgeError
//! - `edgeforge-runtime` — InferenceEngine (for quarantine reader)
//! - `regex` — injection pattern matching
//! - `base64` — data encapsulation
//! - `once_cell` — lazy static patterns
//!
//! ## Architectural Invariants
//! - Opt-in per agent (simple chat uses single-LLM fast path)
//! - Prompt content is NEVER stored in audit tables (migration 0012)

pub mod guardrails;
pub mod privilege;
pub mod quarantine;
pub mod spotlighting;
pub use guardrails::{
    DefaultIntentGuardrails, GuardrailVerdict, IntentClassifier, IntentGuardrails, IntentViolation,
};
pub use privilege::{
    ActionPlan, DecodedAction, DualLlmPipeline, PlannerLlm, PrivilegeError, PrivilegeLevel,
    WorkerLlm,
};
pub use quarantine::{
    DefaultQuarantineReader, QuarantineError, QuarantineReader, SanitizedSummary,
};
pub use spotlighting::{Spotlighter, SpotlightingConfig, SpotlightingTechnique};
