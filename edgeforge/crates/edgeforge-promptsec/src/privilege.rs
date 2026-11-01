// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Dual-LLM Privilege Separation (ADR-020)
//!
//! ## Purpose
//! Splits planning from execution to break the prompt-injection authority chain.
//! The privileged planner sees the full prompt but cannot invoke tools. The
//! unprivileged worker can invoke tools but only sees spotlighted, decontextualized
//! fragments — never the original prompt.
//!
//! ## What This File Does
//! - `PrivilegeLevel` — Privileged (planner) / Unprivileged (worker)
//! - `ActionPlan` — structured output from planner (intent_summary, steps,
//!   permitted_tools, external_resources)
//! - `DecodedAction` — worker's output (tool, arguments, plan_step_index)
//! - `PlannerLlm` trait — `plan(prompt) -> ActionPlan`
//! - `WorkerLlm` trait — `execute_step(plan, step, ctx) -> DecodedAction`
//! - `DualLlmPipeline` — orchestrates the full pipeline with guardrail checks
//! - `PrivilegeError` — error enum (PlannerFailed, WorkerFailed, PlanDeviation, etc.)
//!
//! ## Dependencies
//! - `edgeforge-core` — Prompt, EdgeForgeError
//! - `crate::spotlighting::Spotlighter` — for spotlighting summaries
//! - `crate::guardrails::IntentGuardrails` — for pre/post checks
//! - `crate::quarantine::QuarantineReader` — for external content sanitization
//!
//! ## Output
//! `Result<DecodedAction, PrivilegeError>` — the validated tool call to execute.
//!
//! ## Function: DualLlmPipeline::run
//! - **Purpose:** Run the full dual-LLM pipeline for a user prompt
//! - **Parameters:** `&Prompt` — the user's prompt
//! - **Returns:** `Result<DecodedAction, PrivilegeError>` — validated action
//! - **Errors:** PlannerFailed if guardrail rejects or planner fails;
//!   ToolNotPermitted if worker requests unpermitted tool;
//!   PlanDeviation if worker deviates from plan step
//! - **Dependencies:** guardrails.classify_intent, planner.plan, quarantine.read_and_summarize,
//!   spotlighter.spotlight, worker.execute_step, plan.permitted_tools check

use crate::spotlighting::Spotlighter;
use async_trait::async_trait;
use edgeforge_core::{EdgeForgeError, Prompt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `PrivilegeLevel` enumeration — see module documentation for variants.
pub enum PrivilegeLevel {
    Privileged,
    Unprivileged,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `ActionPlan` — see module documentation for purpose and usage.
pub struct ActionPlan {
    pub intent_summary: String,
    pub steps: Vec<PlanStep>,
    pub permitted_tools: Vec<String>,
    pub external_resources: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `PlanStep` — see module documentation for purpose and usage.
pub struct PlanStep {
    pub description: String,
    pub tool: Option<String>,
    pub arguments: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `DecodedAction` — see module documentation for purpose and usage.
pub struct DecodedAction {
    pub tool: String,
    pub arguments: serde_json::Value,
    pub plan_step_index: usize,
}
#[async_trait]
/// The `PlannerLlm` trait — see module documentation for the contract.
pub trait PlannerLlm: Send + Sync + 'static {
    async fn plan(&self, prompt: &Prompt) -> Result<ActionPlan, PrivilegeError>;
}
#[async_trait]
/// The `WorkerLlm` trait — see module documentation for the contract.
pub trait WorkerLlm: Send + Sync + 'static {
    async fn execute_step(
        &self,
        plan: &ActionPlan,
        step_index: usize,
        ctx: &str,
    ) -> Result<DecodedAction, PrivilegeError>;
}
#[derive(Debug, thiserror::Error)]
/// `PrivilegeError` enumeration — see module documentation for variants.
pub enum PrivilegeError {
    #[error("planner failed: {0}")]
    PlannerFailed(String),
    #[error("worker failed: {0}")]
    WorkerFailed(String),
    #[error("plan deviation: expected {expected}, got {got}")]
    PlanDeviation { expected: usize, got: usize },
    #[error("tool not permitted: {tool}")]
    ToolNotPermitted { tool: String },
    #[error("malformed plan: {0}")]
    MalformedPlan(String),
}
impl From<PrivilegeError> for EdgeForgeError {
    fn from(e: PrivilegeError) -> Self {
        EdgeForgeError::SandboxDenied(e.to_string())
    }
}
impl From<crate::quarantine::QuarantineError> for PrivilegeError {
    fn from(e: crate::quarantine::QuarantineError) -> Self {
        Self::PlannerFailed(format!("quarantine: {e}"))
    }
}
/// `DualLlmPipeline` — see module documentation for purpose and usage.
pub struct DualLlmPipeline {
    pub planner: Arc<dyn PlannerLlm>,
    pub worker: Arc<dyn WorkerLlm>,
    pub guardrails: Arc<dyn crate::IntentGuardrails>,
    pub quarantine: Arc<dyn crate::QuarantineReader>,
    pub spotlighter: Spotlighter,
}
impl DualLlmPipeline {
    /// Creates a new instance with default configuration.
    pub fn new(
        planner: Arc<dyn PlannerLlm>,
        worker: Arc<dyn WorkerLlm>,
        guardrails: Arc<dyn crate::IntentGuardrails>,
        quarantine: Arc<dyn crate::QuarantineReader>,
        spotlighter: Spotlighter,
    ) -> Self {
        Self {
            planner,
            worker,
            guardrails,
            quarantine,
            spotlighter,
        }
    }
    /// Executes the main logic of this function.
    pub async fn run(&self, prompt: &Prompt) -> Result<DecodedAction, PrivilegeError> {
        let pre = self.guardrails.classify_intent(prompt).await;
        if let crate::GuardrailVerdict::Reject(v) = pre {
            return Err(PrivilegeError::PlannerFailed(format!("rejected: {v}")));
        }
        let plan = self.planner.plan(prompt).await?;
        let mut ctx = String::new();
        for res in &plan.external_resources {
            let s = self
                .quarantine
                .read_and_summarize(res, &plan.intent_summary)
                .await?;
            ctx.push_str(&self.spotlighter.spotlight(&s.text));
            ctx.push('\n');
        }
        let action = self.worker.execute_step(&plan, 0, &ctx).await?;
        if !plan.permitted_tools.contains(&action.tool) {
            return Err(PrivilegeError::ToolNotPermitted { tool: action.tool });
        }
        if action.plan_step_index != 0 {
            return Err(PrivilegeError::PlanDeviation {
                expected: 0,
                got: action.plan_step_index,
            });
        }
        Ok(action)
    }
}
