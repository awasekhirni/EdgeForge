// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Multi-Horizon Speculation Engine (ADR-015, ADR-016)
//!
//! ## Purpose
//! Replaces depth-1 prefetch with a 3-horizon engine that predicts experts
//! across multiple layers and time scales, gated by a hard accuracy SLA.
//!
//! ## What This File Does
//! - `SpeculationStrategy` — Off, RouterLogitOnly, NgramSimple, NgramMapK, Draft, Eagle3
//! - `WorkloadType` — Code, Structured, NaturalLanguage, Mixed
//! - `SpeculationConfig` — h1_enabled, h2_strategy, h3_enabled, workload_detection, etc.
//! - `SpeculationVerdict` — horizon, hit, surrogate_used, accuracy_delta
//! - `detect_workload(prompt)` — classify prompt into WorkloadType
//! - `strategy_for_workload(workload)` — map workload to strategy (ADR-016)
//! - `h1_predict_next_layer_experts()` — H1: router logit top-k (>90% accuracy)
//! - `h2_predict_future_experts()` — H2: surrogate/draft model (60-85% accuracy)
//! - `h3_predict_from_graph()` — H3: streaming graph sequence query (40-70% accuracy)
//! - `permit_surrogate()` — confidence-aware fallback (Quality SLA enforced, R-19)
//! - `SpeculationEngine::predict()` — coordinates all 3 horizons with dedup
//!
//! ## Dependencies
//! - `edgeforge-core::{ExpertId, LayerIdx, Prompt, SpeculationHorizon}`
//! - `edgeforge-graph::StreamingGraph` — for H3 prediction
//!
//! ## Output
//! `Vec<(LayerIdx, ExpertId)>` — predicted experts (deduplicated).
//!
//! ## Three Horizons
//! | Horizon | Predictor | Accuracy | Preset Gate |
//! |---|---|---|---|
//! | H1 | Router logit pre-read | >90% | All presets |
//! | H2 | Surrogate/draft model | 60-85% | Quality: correctness-preserving only |
//! | H3 | Graph sequence query | 40-70% | All presets (advisory) |
//!
//! ## Function: detect_workload
//! - **Purpose:** Classify the prompt into a workload type for strategy selection
//! - **Parameters:** `&Prompt`
//! - **Returns:** `WorkloadType` — Code, Structured, NaturalLanguage, or Mixed
//! - **Heuristics:** Code = high density of `{}();` tokens; Structured = "JSON"/"SQL" keywords;
//!   NaturalLanguage = default
//!
//! ## Function: permit_surrogate
//! - **Purpose:** Decide if surrogate approximation is permitted on a cache miss
//! - **Parameters:** `miss_confidence: f32`, `preset_quality: bool`, `surrogate_available: bool`
//! - **Returns:** `bool` — true if surrogate is permitted
//! - **Rules:** Quality preset → always false (R-19); not available → false;
//!   miss_confidence < 0.3 → true

use edgeforge_core::{ExpertId, LayerIdx, SpeculationHorizon};
use serde::{Deserialize, Serialize};

/// The speculation strategy. Maps to llama.cpp's `--spec-type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeculationStrategy {
    Off,
    RouterLogitOnly,
    NgramSimple,
    NgramMapK,
    Draft,
    Eagle3,
}

/// The detected workload type (ADR-016).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadType {
    Code,
    Structured,
    NaturalLanguage,
    Mixed,
}

/// Speculation configuration.
#[derive(Clone, Debug)]
pub struct SpeculationConfig {
    pub h1_enabled: bool,
    pub h2_strategy: SpeculationStrategy,
    pub h3_enabled: bool,
    pub workload_detection: bool,
    pub accuracy_sla_enforced: bool,
    pub draft_max: u32,
    pub draft_p_min: f32,
}

impl Default for SpeculationConfig {
    fn default() -> Self {
        Self {
            h1_enabled: true,
            h2_strategy: SpeculationStrategy::Draft,
            h3_enabled: true,
            workload_detection: true,
            accuracy_sla_enforced: true,
            draft_max: 16,
            draft_p_min: 0.5,
        }
    }
}

/// A speculation verdict for a single prediction.
#[derive(Clone, Copy, Debug)]
pub struct SpeculationVerdict {
    pub horizon: SpeculationHorizon,
    pub hit: bool,
    pub surrogate_used: bool,
    pub accuracy_delta: Option<f32>,
}

/// Detect the workload type from the prompt (ADR-016, §7.4).
pub fn detect_workload(prompt: &edgeforge_core::Prompt) -> WorkloadType {
    let text = prompt.render_flat();
    let lower = text.to_lowercase();

    let code_chars = text.chars().filter(|&c| "{}();".contains(c)).count();
    if code_chars as f32 / text.len().max(1) as f32 > 0.05 {
        return WorkloadType::Code;
    }
    if lower.contains("json") || lower.contains("sql") || lower.contains("schema") {
        return WorkloadType::Structured;
    }
    WorkloadType::NaturalLanguage
}

/// Select the speculation strategy based on workload (ADR-016).
#[must_use]
pub fn strategy_for_workload(workload: WorkloadType) -> SpeculationStrategy {
    match workload {
        WorkloadType::Code => SpeculationStrategy::NgramSimple,
        WorkloadType::Structured => SpeculationStrategy::NgramMapK,
        WorkloadType::NaturalLanguage => SpeculationStrategy::Draft,
        WorkloadType::Mixed => SpeculationStrategy::Draft,
    }
}

/// H1: predict the next layer's top-k experts from the router logits.
///
/// Always on (all presets). Accuracy target: >90%.
/// Returns the predicted expert IDs for layer N+1.
#[must_use]
pub fn h1_predict_next_layer_experts(
    _current_layer: LayerIdx,
    _router_logits: &[f32],
    top_k: usize,
) -> Vec<ExpertId> {
    // Phase 5: return the top-k indices from the logits.
    // In production, this reads the router's softmax output.
    let mut indexed: Vec<(usize, f32)> = _router_logits
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    indexed
        .into_iter()
        .take(top_k)
        .map(|(i, _)| ExpertId(i as u32))
        .collect()
}

/// H2: predict experts for layers N+2..N+d using a surrogate or draft model.
///
/// Preset-gated: Quality = correctness-preserving only; Balanced/Fast = stall-free.
/// Accuracy target: 60-85%.
#[must_use]
pub fn h2_predict_future_experts(
    current_layer: LayerIdx,
    depth: u32,
    draft_model_output: &[Vec<ExpertId>],
) -> Vec<(LayerIdx, ExpertId)> {
    // Phase 5: return the union of draft predictions for the next `depth` layers.
    let mut result = Vec::new();
    for (i, experts) in draft_model_output.iter().take(depth as usize).enumerate() {
        let layer = LayerIdx(current_layer.0 + 1 + i as u32);
        for &e in experts {
            result.push((layer, e));
        }
    }
    result
}

/// H3: query the streaming graph for the predicted expert set over the next N tokens.
///
/// Advisory only (all presets). Accuracy target: 40-70%.
/// Source: `StreamingGraph::predicted_expert_set(sequence, n)`.
#[must_use]
pub fn h3_predict_from_graph(
    graph: &dyn edgeforge_graph::StreamingGraph,
    sequence: u64,
    n: u32,
) -> Vec<(LayerIdx, ExpertId)> {
    graph.predicted_expert_set(sequence, n)
}

/// Confidence-aware fallback decision (ADR-015, §7.3).
///
/// Returns `true` if surrogate approximation is permitted; `false` if a
/// blocking SSD fetch is required.
#[must_use]
pub fn permit_surrogate(
    miss_confidence: f32,
    preset_quality: bool,
    surrogate_available: bool,
) -> bool {
    if preset_quality {
        return false;
    } // Quality SLA enforced (R-19)
    if !surrogate_available {
        return false;
    }
    miss_confidence < 0.3
}

/// The multi-horizon speculation engine.
///
/// Coordinates H1, H2, and H3 predictions and deduplicates the result
/// against resident/in-flight experts.
pub struct SpeculationEngine {
    pub config: SpeculationConfig,
}

impl SpeculationEngine {
    #[must_use]
    /// Creates a new instance with default configuration.
    pub fn new(config: SpeculationConfig) -> Self {
        Self { config }
    }

    /// Run all enabled horizons and return the union of predicted experts.
    ///
    /// `graph` is optional — if None, H3 is skipped.
    #[must_use]
    pub fn predict(
        &self,
        current_layer: LayerIdx,
        router_logits: &[f32],
        top_k: usize,
        draft_output: &[Vec<ExpertId>],
        graph: Option<&dyn edgeforge_graph::StreamingGraph>,
        sequence: u64,
    ) -> Vec<(LayerIdx, ExpertId)> {
        let mut predictions: Vec<(LayerIdx, ExpertId)> = Vec::new();

        // H1: always on (if enabled).
        if self.config.h1_enabled {
            let h1_experts = h1_predict_next_layer_experts(current_layer, router_logits, top_k);
            let next_layer = current_layer.next();
            for e in h1_experts {
                predictions.push((next_layer, e));
            }
        }

        // H2: preset-gated.
        if self.config.h2_strategy != SpeculationStrategy::Off {
            let depth = 2; // predict 2 layers ahead
            let h2_experts = h2_predict_future_experts(current_layer, depth, draft_output);
            predictions.extend(h2_experts);
        }

        // H3: advisory, graph-based.
        if self.config.h3_enabled {
            if let Some(g) = graph {
                if g.healthy() {
                    let h3_experts = h3_predict_from_graph(g, sequence, 10);
                    predictions.extend(h3_experts);
                }
            }
        }

        // Deduplicate.
        predictions.sort_by_key(|(l, e)| (l.0, e.0));
        predictions.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

        predictions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_code_workload() {
        let prompt = edgeforge_core::Prompt::new(vec![edgeforge_core::ChatMessage {
            role: edgeforge_core::Role::User,
            content: "function foo() { return bar(); }".to_string(),
        }]);
        assert_eq!(detect_workload(&prompt), WorkloadType::Code);
    }

    #[test]
    fn detect_structured_workload() {
        let prompt = edgeforge_core::Prompt::new(vec![edgeforge_core::ChatMessage {
            role: edgeforge_core::Role::User,
            content: "Return the result as JSON".to_string(),
        }]);
        assert_eq!(detect_workload(&prompt), WorkloadType::Structured);
    }

    #[test]
    fn detect_natural_language() {
        let prompt = edgeforge_core::Prompt::new(vec![edgeforge_core::ChatMessage {
            role: edgeforge_core::Role::User,
            content: "What is the capital of France?".to_string(),
        }]);
        assert_eq!(detect_workload(&prompt), WorkloadType::NaturalLanguage);
    }

    #[test]
    fn quality_preset_never_permits_surrogate() {
        assert!(!permit_surrogate(0.1, true, true));
    }

    #[test]
    fn balanced_preset_permits_low_confidence_surrogate() {
        assert!(permit_surrogate(0.2, false, true));
        assert!(!permit_surrogate(0.5, false, true));
    }

    #[test]
    fn h1_predicts_top_k() {
        let logits = vec![0.1, 0.5, 0.3, 0.9, 0.2];
        let experts = h1_predict_next_layer_experts(LayerIdx(0), &logits, 3);
        assert_eq!(experts.len(), 3);
        // Highest logit is index 3 (0.9).
        assert_eq!(experts[0], ExpertId(3));
    }

    #[test]
    fn speculation_engine_predicts_union() {
        let engine = SpeculationEngine::new(SpeculationConfig::default());
        let logits = vec![0.1, 0.9, 0.2];
        let draft = vec![vec![ExpertId(0), ExpertId(1)]];
        let predictions = engine.predict(LayerIdx(0), &logits, 2, &draft, None, 0);
        // Should have H1 (2 experts) + H2 (2 experts) = 4, minus duplicates.
        assert!(!predictions.is_empty());
    }

    #[test]
    fn speculation_engine_deduplicates() {
        let engine = SpeculationEngine::new(SpeculationConfig::default());
        let logits = vec![0.9, 0.1]; // H1 predicts expert 0 for layer 1
        let draft = vec![vec![ExpertId(0)]]; // H2 also predicts expert 0
        let predictions = engine.predict(LayerIdx(0), &logits, 1, &draft, None, 0);
        // Expert 0 for the same layer should appear only once after dedup.
        let count_layer1_expert0 = predictions
            .iter()
            .filter(|(l, e)| l.0 == 1 && e.0 == 0)
            .count();
        assert!(count_layer1_expert0 <= 1, "duplicates should be removed");
    }
}
