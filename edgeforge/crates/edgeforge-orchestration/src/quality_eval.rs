// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Quality-vs-Speed Tradeoff Report (F-MON-02)
//!
//! ## Purpose
//! Runs a small evaluation suite (MMLU-lite, HumanEval-10) to measure
//! the quality impact of different presets and quantization levels.
//!
//! ## What This File Does
//! - `QualityEvalResult` — result with MMLU-lite and HumanEval-10 scores
//! - `run_quality_eval()` — run the evaluation using an InferenceEngine
//! - `MMLU_LITE_QUESTIONS` — sample MMLU-lite questions
//! - `HUMANEVAL_PROBLEMS` — sample HumanEval-10 problems
//!
//! ## Dependencies
//! - `edgeforge-core` — Prompt, Role, ChatMessage
//! - `edgeforge-runtime` — InferenceEngine, GenParams, LoadedModel
//! - `edgeforge-observability` — FunctionGuard for call logging
//!
//! ## Output
//! `QualityEvalResult` with scores in [0.0, 1.0].
//!
//! ## Function Logging
//! `timestamp - edgeforge-orchestration - quality_eval.rs - function - STATUS`

use edgeforge_core::{ChatMessage, Prompt, Role};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use edgeforge_runtime::{GenParams, InferenceEngine, LoadedModel};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// A sample MMLU-lite question.
#[derive(Clone, Debug)]
struct MlmuQuestion {
    question: &'static str,
    choices: &'static [&'static str],
    correct: usize,
}

/// Sample MMLU-lite questions (simplified — 5 questions).
const MMLU_LITE_QUESTIONS: &[MlmuQuestion] = &[
    MlmuQuestion {
        question: "What is the capital of France?",
        choices: &["London", "Paris", "Berlin", "Madrid"],
        correct: 1,
    },
    MlmuQuestion {
        question: "Which planet is known as the Red Planet?",
        choices: &["Venus", "Mars", "Jupiter", "Saturn"],
        correct: 1,
    },
    MlmuQuestion {
        question: "What is 2 + 2?",
        choices: &["3", "4", "5", "6"],
        correct: 1,
    },
    MlmuQuestion {
        question: "Who wrote 'Romeo and Juliet'?",
        choices: &["Dickens", "Shakespeare", "Hemingway", "Tolstoy"],
        correct: 1,
    },
    MlmuQuestion {
        question: "What is the chemical symbol for water?",
        choices: &["H2O", "CO2", "NaCl", "O2"],
        correct: 0,
    },
];

/// A sample HumanEval problem.
#[derive(Clone, Debug)]
struct HumanEvalProblem {
    prompt: &'static str,
    expected_contains: &'static str,
}

/// Sample HumanEval-10 problems (simplified — 3 problems).
const HUMANEVAL_PROBLEMS: &[HumanEvalProblem] = &[
    HumanEvalProblem {
        prompt: "Write a function that returns the sum of two numbers.\ndef add(a, b):",
        expected_contains: "return a + b",
    },
    HumanEvalProblem {
        prompt: "Write a function that returns the factorial of n.\ndef factorial(n):",
        expected_contains: "return n *",
    },
    HumanEvalProblem {
        prompt: "Write a function that checks if a string is a palindrome.\ndef is_palindrome(s):",
        expected_contains: "return s == s[::-1]",
    },
];

/// Quality evaluation result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityEvalResult {
    /// The preset that was evaluated.
    pub preset: String,
    /// The precision that was evaluated.
    pub precision: String,
    /// MMLU-lite score in [0.0, 1.0].
    pub mmlu_lite_score: f32,
    /// HumanEval-10 score in [0.0, 1.0].
    pub humaneval_10_score: f32,
    /// Quality delta vs FP16 baseline (positive = better, negative = worse).
    pub quality_delta_vs_fp16: f32,
}

/// Run the quality evaluation suite.
///
/// ## Purpose
/// Runs MMLU-lite and HumanEval-10 evaluations using the given inference engine.
/// Returns scores in [0.0, 1.0] for each benchmark.
///
/// ## Parameters
/// - `engine`: The inference engine to evaluate.
/// - `model`: The loaded model.
/// - `preset`: The preset name (e.g., "quality", "fast").
/// - `precision`: The precision name (e.g., "fp16", "q4").
///
/// ## Returns
/// `Ok(QualityEvalResult)` on success, `Err(EdgeForgeError)` on failure.
///
/// ## Function Logging
/// `timestamp - edgeforge-orchestration - quality_eval.rs - run_quality_eval - STATUS`
pub async fn run_quality_eval(
    engine: &dyn InferenceEngine,
    model: &LoadedModel,
    preset: &str,
    precision: &str,
) -> Result<QualityEvalResult, edgeforge_core::EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-orchestration",
        "quality_eval.rs",
        "run_quality_eval",
    );
    tracing::info!(
        preset = preset,
        precision = precision,
        "Starting quality evaluation"
    );

    let params = GenParams {
        max_tokens: 64,
        temperature: 0.0, // greedy for reproducibility
        ..GenParams::default()
    };

    // Run MMLU-lite.
    let mut mmlu_correct = 0u32;
    for q in MMLU_LITE_QUESTIONS {
        let prompt = Prompt::new(vec![ChatMessage {
            role: Role::User,
            content: format!(
                "Question: {}\nChoices: {}\nAnswer with the number (0-3) of the correct choice.\nAnswer:",
                q.question,
                q.choices
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{i}: {c}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }]);

        let mut response = String::new();
        let _ = engine
            .generate(model, &prompt, &params, &mut |token, _| {
                response.push_str(&token.text);
                true
            })
            .await?;

        // Check if the correct answer index appears in the response.
        let correct_str = q.correct.to_string();
        if response.contains(&correct_str) {
            mmlu_correct += 1;
        }
    }
    let mmlu_score = mmlu_correct as f32 / MMLU_LITE_QUESTIONS.len() as f32;

    // Run HumanEval-10.
    let mut humaneval_correct = 0u32;
    for p in HUMANEVAL_PROBLEMS {
        let prompt = Prompt::new(vec![ChatMessage {
            role: Role::User,
            content: p.prompt.to_string(),
        }]);

        let mut response = String::new();
        let _ = engine
            .generate(model, &prompt, &params, &mut |token, _| {
                response.push_str(&token.text);
                true
            })
            .await?;

        // Check if the expected substring appears in the response.
        if response.contains(p.expected_contains) {
            humaneval_correct += 1;
        }
    }
    let humaneval_score = humaneval_correct as f32 / HUMANEVAL_PROBLEMS.len() as f32;

    // Quality delta vs FP16 baseline (assumes FP16 = 1.0 baseline).
    let baseline = 1.0_f32;
    let delta = ((mmlu_score + humaneval_score) / 2.0) - baseline;

    let result = QualityEvalResult {
        preset: preset.to_string(),
        precision: precision.to_string(),
        mmlu_lite_score: mmlu_score,
        humaneval_10_score: humaneval_score,
        quality_delta_vs_fp16: delta,
    };

    tracing::info!(
        mmlu = mmlu_score,
        humaneval = humaneval_score,
        delta = delta,
        "Quality evaluation complete"
    );
    guard.success();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_crowd::ExecutionMode;
    use edgeforge_runtime::MockInferenceEngine;
    use edgeforge_security::{ModelArch, ModelManifest};

    fn make_manifest() -> ModelManifest {
        ModelManifest {
            schema_version: 1,
            id: edgeforge_core::ModelId::new(),
            name: "Quality Eval Test".to_string(),
            publisher: "test".to_string(),
            license: "Apache-2.0".to_string(),
            license_url: None,
            acceptable_uses: vec![],
            geo_restrictions: vec![],
            file_size_bytes: 1024,
            sha256: [0u8; 32],
            precision: edgeforge_core::Precision::Fp16,
            arch: ModelArch::Moe,
            total_params: 1_000_000_000,
            active_params: Some(50_000_000),
            dense_core_bytes: 100_000_000,
            expert_pool_bytes: 900_000_000,
            signature: None,
            publisher_key_fingerprint: None,
        }
    }

    #[tokio::test]
    async fn test_run_quality_eval_with_mock() {
        let engine = MockInferenceEngine::new();
        let manifest = make_manifest();
        let model = engine.load(&manifest, ExecutionMode::Local).await.unwrap();
        let result = run_quality_eval(&engine, &model, "quality", "fp16")
            .await
            .unwrap();
        assert!(result.mmlu_lite_score >= 0.0 && result.mmlu_lite_score <= 1.0);
        assert!(result.humaneval_10_score >= 0.0 && result.humaneval_10_score <= 1.0);
    }

    #[test]
    fn test_quality_eval_result_serialize() {
        let r = QualityEvalResult {
            preset: "quality".to_string(),
            precision: "fp16".to_string(),
            mmlu_lite_score: 0.8,
            humaneval_10_score: 0.6,
            quality_delta_vs_fp16: 0.0,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("quality"));
        assert!(json.contains("fp16"));
    }
}
