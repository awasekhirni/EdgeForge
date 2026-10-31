// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::privilege::{ActionPlan, DecodedAction};
use async_trait::async_trait;
use edgeforge_core::Prompt;
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuardrailVerdict {
    Allow,
    Reject(IntentViolation),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
pub trait IntentGuardrails: Send + Sync + 'static {
    async fn classify_intent(&self, prompt: &Prompt) -> GuardrailVerdict;
    async fn validate_action(&self, action: &DecodedAction, plan: &ActionPlan) -> GuardrailVerdict;
}
#[async_trait]
pub trait IntentClassifier: Send + Sync + 'static {
    async fn classify(&self, prompt: &Prompt) -> GuardrailVerdict;
}
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
pub struct RegexIntentClassifier {
    patterns: RwLock<Vec<(Regex, ViolationCategory)>>,
}
impl Default for RegexIntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}
impl RegexIntentClassifier {
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
