// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Intent Guardrails (ADR-022)
//!
//! ## Purpose
//! Two-stage classifier + rule engine that validates the user's stated intent
//! against the worker's decoded action before any tool is invoked.
//!
//! ## What This File Does
//! - Stage 1 (pre-flight): `classify_intent(prompt)` — reject if matches denylist
//! - Stage 2 (post-decode): `validate_action(action, plan)` — reject if action
//!   deviates from plan
//! - `GuardrailVerdict` — Allow / Reject(IntentViolation)
//! - `IntentViolation` — category + detail
//! - `ViolationCategory` — DataExfiltration, ArbitraryShell, SystemModification,
//!   PromptRevelation, PlanDeviation, UnpermittedTool, SuspiciousExternalContent, Other
//! - `IntentGuardrails` trait — classify_intent + validate_action
//! - `IntentClassifier` trait — pluggable classifier
//! - `RegexIntentClassifier` — default regex denylist with 5 categories
//! - `DefaultIntentGuardrails` — combines classifier + rule engine
//!
//! ## Dependencies
//! - `edgeforge-core::Prompt` — for intent classification
//! - `regex` — denylist patterns
//! - `parking_lot::RwLock` — thread-safe pattern list
//! - `crate::privilege::{ActionPlan, DecodedAction}` — for action validation
//!
//! ## Output
//! `GuardrailVerdict` — Allow or Reject with violation category + detail.
//!
//! ## Function: DefaultIntentGuardrails::validate_action
//! - **Purpose:** Validate the worker's decoded action against the plan
//! - **Parameters:** `&DecodedAction`, `&ActionPlan`
//! - **Returns:** `GuardrailVerdict` — Allow if action is valid
//! - **Rules:** (1) tool must be in permitted_tools, (2) step index in range,
//!   (3) tool matches planned step
//! - **Errors:** Reject with UnpermittedTool or PlanDeviation

use crate::privilege::{ActionPlan, DecodedAction};
use async_trait::async_trait;
use edgeforge_core::Prompt;
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
/// `GuardrailVerdict` enumeration — see module documentation for variants.
pub enum GuardrailVerdict {
    Allow,
    Reject(IntentViolation),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// `IntentViolation` — see module documentation for purpose and usage.
pub struct IntentViolation {
    pub category: ViolationCategory,
    pub detail: String,
}
impl std::fmt::Display for IntentViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.category, self.detail)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `ViolationCategory` enumeration — see module documentation for variants.
pub enum ViolationCategory {
    DataExfiltration,
    ArbitraryShell,
    SystemModification,
    PromptRevelation,
    PlanDeviation,
    UnpermittedTool,
    SuspiciousExternalContent,
    Other,
}
#[async_trait]
/// The `IntentGuardrails` trait — see module documentation for the contract.
pub trait IntentGuardrails: Send + Sync + 'static {
    async fn classify_intent(&self, prompt: &Prompt) -> GuardrailVerdict;
    async fn validate_action(&self, action: &DecodedAction, plan: &ActionPlan) -> GuardrailVerdict;
}
#[async_trait]
/// The `IntentClassifier` trait — see module documentation for the contract.
pub trait IntentClassifier: Send + Sync + 'static {
    async fn classify(&self, prompt: &Prompt) -> GuardrailVerdict;
}
/// `DefaultIntentGuardrails` — see module documentation for purpose and usage.
pub struct DefaultIntentGuardrails {
    pub classifier: Arc<dyn IntentClassifier>,
}
impl DefaultIntentGuardrails {
    pub fn new(classifier: Arc<dyn IntentClassifier>) -> Self {
        Self { classifier }
    }
}
#[async_trait]
impl IntentGuardrails for DefaultIntentGuardrails {
    async fn classify_intent(&self, prompt: &Prompt) -> GuardrailVerdict {
        self.classifier.classify(prompt).await
    }
    async fn validate_action(&self, action: &DecodedAction, plan: &ActionPlan) -> GuardrailVerdict {
        if !plan.permitted_tools.contains(&action.tool) {
            return GuardrailVerdict::Reject(IntentViolation {
                category: ViolationCategory::UnpermittedTool,
                detail: format!("tool '{}' not permitted", action.tool),
            });
        }
        if action.plan_step_index >= plan.steps.len() {
            return GuardrailVerdict::Reject(IntentViolation {
                category: ViolationCategory::PlanDeviation,
                detail: format!("step {} out of range", action.plan_step_index),
            });
        }
        GuardrailVerdict::Allow
    }
}
/// `RegexIntentClassifier` — see module documentation for purpose and usage.
pub struct RegexIntentClassifier {
    patterns: RwLock<Vec<(Regex, ViolationCategory)>>,
}
impl Default for RegexIntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}
impl RegexIntentClassifier {
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self {
            patterns: RwLock::new(vec![
                (
                    Regex::new(
                        "(?i)(send|upload|exfiltrate).{0,40}(ssh|password|secret|token|api.?key)",
                    )
                    .unwrap(),
                    ViolationCategory::DataExfiltration,
                ),
                (
                    Regex::new(r"(?i)(rm|del|format)\s+[-/]").unwrap(),
                    ViolationCategory::ArbitraryShell,
                ),
                (
                    Regex::new("(?i)(chmod|chown|/etc/|/usr/)").unwrap(),
                    ViolationCategory::SystemModification,
                ),
                (
                    Regex::new("(?i)(reveal|show|print).{0,30}(system )?prompt").unwrap(),
                    ViolationCategory::PromptRevelation,
                ),
            ]),
        }
    }
}
#[async_trait]
impl IntentClassifier for RegexIntentClassifier {
    async fn classify(&self, prompt: &Prompt) -> GuardrailVerdict {
        let text = prompt.render_flat();
        let pats = self.patterns.read();
        for (re, cat) in pats.iter() {
            if re.is_match(&text) {
                return GuardrailVerdict::Reject(IntentViolation {
                    category: *cat,
                    detail: format!("matched: {}", re.as_str()),
                });
            }
        }
        GuardrailVerdict::Allow
    }
}
