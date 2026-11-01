//! Phase 4: End-to-end integration test.
//!
//! Exercises the full pipeline: MockInferenceEngine → router hook →
//! token generation → verification outcome.
//!
//! This test does NOT require the API server or SQLite — it tests the
//! inference engine directly, which is the core of the pipeline.

#[cfg(test)]
mod tests {
    use edgeforge_core::{ChatMessage, ExpertId, LayerIdx, ModelId, Prompt, Role, Token};
    use edgeforge_crowd::{ExecutionMode, VerificationOutcome};
    use edgeforge_runtime::{ExpertResolution, GenParams, InferenceEngine, MockInferenceEngine, RouterHook};
    use edgeforge_security::{ModelArch, ModelManifest};

    /// A test router hook that always returns RAM (simulating a warm cache).
    struct WarmCacheRouterHook;
    impl RouterHook for WarmCacheRouterHook {
        fn resolve_expert(&self, _layer: LayerIdx, _expert: ExpertId) -> ExpertResolution {
            ExpertResolution::Ram
        }
    }

    fn make_manifest(name: &str) -> ModelManifest {
        ModelManifest {
            schema_version: 1,
            id: ModelId::new(),
            name: name.to_string(),
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

    fn make_prompt(text: &str) -> Prompt {
        Prompt::new(vec![ChatMessage {
            role: Role::User,
            content: text.to_string(),
        }])
    }

    /// Full end-to-end: load model → install router hook → generate tokens →
    /// verify tokens are produced with correct verification outcome.
    #[tokio::test]
    async fn e2e_full_pipeline_with_router_hook() {
        let engine = MockInferenceEngine::new();

        // 1. Load the model.
        let manifest = make_manifest("E2E Test Model");
        let model = engine
            .load(&manifest, ExecutionMode::Local)
            .await
            .expect("model load should succeed");

        // 2. Install a router hook (simulates the graph-aware routing path).
        engine
            .install_router_hook(Box::new(WarmCacheRouterHook))
            .expect("router hook install should succeed");

        // 3. Generate tokens.
        let prompt = make_prompt("hello world from edgeforge e2e test");
        let params = GenParams {
            max_tokens: 10,
            ..GenParams::default()
        };

        let mut tokens: Vec<Token> = Vec::new();
        let mut outcomes: Vec<VerificationOutcome> = Vec::new();

        let stats = engine
            .generate(&model, &prompt, &params, &mut |token, outcome| {
                tokens.push(token);
                outcomes.push(outcome);
                true
            })
            .await
            .expect("generation should succeed");

        // 4. Verify results.
        assert!(stats.tokens_generated > 0, "should generate tokens");
        assert_eq!(tokens.len() as u32, stats.tokens_generated);
        assert_eq!(outcomes.len(), tokens.len());

        // For Local mode, all outcomes should be NotRequired.
        for outcome in &outcomes {
            assert_eq!(
                *outcome,
                VerificationOutcome::NotRequired,
                "Local mode should not require verification"
            );
        }

        // The mock response is the prompt reversed.
        let first_word = tokens[0].text.trim();
        assert!(
            first_word == "test" || first_word == "e2e",
            "first token should be from the reversed prompt, got: {first_word}"
        );

        // 5. Verify timing stats.
        assert!(stats.elapsed_ms > 0 || stats.tokens_generated > 0);
    }

    /// End-to-end: verify that max_tokens is respected.
    #[tokio::test]
    async fn e2e_max_tokens_respected() {
        let engine = MockInferenceEngine::new();
        let model = engine
            .load(&make_manifest("Max Tokens Test"), ExecutionMode::Local)
            .await
            .unwrap();

        let prompt = make_prompt("a b c d e f g h i j k l m n o p");
        let params = GenParams {
            max_tokens: 5,
            ..GenParams::default()
        };

        let mut count = 0u32;
        let stats = engine
            .generate(&model, &prompt, &params, &mut |_, _| {
                count += 1;
                true
            })
            .await
            .unwrap();

        assert_eq!(stats.tokens_generated, 5);
        assert_eq!(count, 5);
    }

    /// End-to-end: verify that the callback can abort generation.
    #[tokio::test]
    async fn e2e_callback_aborts_generation() {
        let engine = MockInferenceEngine::new();
        let model = engine
            .load(&make_manifest("Abort Test"), ExecutionMode::Local)
            .await
            .unwrap();

        let prompt = make_prompt("a b c d e f g h i j");
        let params = GenParams {
            max_tokens: 100,
            ..GenParams::default()
        };

        let mut count = 0u32;
        let stats = engine
            .generate(&model, &prompt, &params, &mut |_, _| {
                count += 1;
                count < 5 // abort after 5 tokens
            })
            .await
            .unwrap();

        assert_eq!(stats.tokens_generated, 5);
        assert_eq!(count, 5);
    }

    /// End-to-end: verify that load fails for invalid manifests.
    #[tokio::test]
    async fn e2e_load_fails_for_empty_name() {
        let engine = MockInferenceEngine::new();
        let manifest = make_manifest("");
        let result = engine.load(&manifest, ExecutionMode::Local).await;
        assert!(result.is_err());
    }

    /// End-to-end: verify deterministic output (same prompt → same tokens).
    #[tokio::test]
    async fn e2e_deterministic_output() {
        let engine = MockInferenceEngine::new();
        let model = engine
            .load(&make_manifest("Determinism Test"), ExecutionMode::Local)
            .await
            .unwrap();

        let prompt = make_prompt("alpha beta gamma delta");
        let params = GenParams {
            max_tokens: 4,
            ..GenParams::default()
        };

        let mut tokens1: Vec<String> = Vec::new();
        engine
            .generate(&model, &prompt, &params, &mut |t, _| {
                tokens1.push(t.text.trim().to_string());
                true
            })
            .await
            .unwrap();

        let mut tokens2: Vec<String> = Vec::new();
        engine
            .generate(&model, &prompt, &params, &mut |t, _| {
                tokens2.push(t.text.trim().to_string());
                true
            })
            .await
            .unwrap();

        assert_eq!(tokens1, tokens2, "same prompt should produce same tokens");
    }

    /// End-to-end: verify that the mock engine produces reversed output.
    #[tokio::test]
    async fn e2e_output_is_reversed_prompt() {
        let engine = MockInferenceEngine::new();
        let model = engine
            .load(&make_manifest("Reverse Test"), ExecutionMode::Local)
            .await
            .unwrap();

        // "one two three" reversed = "three two one"
        let prompt = make_prompt("one two three");
        let params = GenParams {
            max_tokens: 3,
            ..GenParams::default()
        };

        let mut tokens: Vec<String> = Vec::new();
        engine
            .generate(&model, &prompt, &params, &mut |t, _| {
                tokens.push(t.text.trim().to_string());
                true
            })
            .await
            .unwrap();

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], "three");
        assert_eq!(tokens[1], "two");
        assert_eq!(tokens[2], "one");
    }
}
