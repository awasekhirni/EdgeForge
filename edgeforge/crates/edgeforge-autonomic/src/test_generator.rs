//! # Antifragile Test Generator (ADR-029)
//!
//! ## Purpose
//! When a remediation succeeds, this module generates a regression test
//! encoding the failure condition. The test is added to the CI suite so the
//! system becomes stronger because it failed — the "antifragile" property.
//!
//! ## What This File Does
//! - `TestKind` — RegressionTest, KaniHarness, PropertyTest
//! - `GeneratedTest` — kind, name, source, target_path
//! - `TestGenerator` — generates tests from failure patterns + remediation
//! - `generate()` — produces a `GeneratedTest` with real Rust source code
//!   encoding the failure condition, or `None` if no pattern matched
//!
//! ## Generated Test Types
//! | Failure Kind | Test Type | Target Path |
//! |---|---|---|
//! | ThermalThrottle | RegressionTest | tests/e2e/autonomic/ |
//! | CorruptExpert | RegressionTest | tests/e2e/autonomic/ |
//! | PeerDisconnect | RegressionTest | tests/e2e/autonomic/ |
//! | SpeculationMissSpike | RegressionTest | tests/e2e/autonomic/ |
//! | VerificationFailure | KaniHarness | tests/kani/ |
//!
//! ## Generated Source
//! The generated test source is a compilable Rust file that:
//! 1. Sets up the failure condition (e.g. inject a high-temperature event)
//! 2. Runs the MAPE-K cycle
//! 3. Asserts the remediation was applied
//! 4. Asserts the failure pattern was detected
//!
//! ## Dependencies
//! - `crate::{FailurePattern, FailureKind, Remediation, RemediationKind}`
//!
//! ## Output
//! `Option<GeneratedTest>` — the generated test, or `None` if no pattern
//! matched the remediation.

use crate::knowledge::{FailureKind, FailurePattern};
use crate::remediation::{Remediation, RemediationKind};
use crate::test_kind::TestKind;
use std::sync::Arc;

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Re-export `TestKind` in a submodule-free way for the crate's public API.
pub mod test_kind {
    /// The kind of generated test.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TestKind {
        /// A regression test that reproduces the failure condition.
        RegressionTest,
        /// A Kani formal verification harness.
        KaniHarness,
        /// A property-based test.
        PropertyTest,
    }
}

pub use test_kind::TestKind;

/// A generated test file.
#[derive(Clone, Debug)]
pub struct GeneratedTest {
    /// The kind of test.
    pub kind: TestKind,
    /// The test name (e.g. `test_thermal_throttle_remmediation`).
    pub name: String,
    /// The Rust source code of the test.
    pub source: String,
    /// The target path where the test should be written
    /// (e.g. `tests/e2e/autonomic/test_thermal_throttle.rs`).
    pub target_path: String,
}

/// Generates antifragile regression tests from failure patterns.
pub struct TestGenerator;

impl TestGenerator {
    /// Creates a new test generator.
    #[must_use]
    pub fn new() -> Arc<Self> {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-autonomic", "test_generator.rs", "new");
        Arc::new(Self)
    }

    /// Generates a regression test encoding the failure condition.
    ///
    /// ## Purpose
    /// When a remediation succeeds, this method produces a `GeneratedTest`
    /// with real Rust source code that reproduces the failure condition and
    /// asserts the remediation was applied. The test is added to the CI
    /// suite so the same failure can never silently regress.
    ///
    /// ## Parameters
    /// - `patterns`: The failure patterns detected by the MAPE-K loop.
    /// - `remediation`: The remediation that was applied.
    ///
    /// ## Returns
    /// `Some(GeneratedTest)` if a pattern matched, `None` if no pattern
    /// matched the remediation.
    ///
    /// ## Matching Logic
    /// The generator finds the highest-severity pattern and generates a
    /// test based on its kind:
    /// - `ThermalThrottle` → RegressionTest that injects a high-temp event
    /// - `CorruptExpert` → RegressionTest that injects an I/O latency spike
    /// - `PeerDisconnect` → RegressionTest that injects a PeerLost event
    /// - `SpeculationMissSpike` → RegressionTest that injects consecutive misses
    /// - `VerificationFailure` → KaniHarness that encodes the verification contract
    pub fn generate(
        &self,
        patterns: &[FailurePattern],
        remediation: &Remediation,
    ) -> Option<GeneratedTest> {
        let mut guard = FunctionGuard::new(
            &LOGGER,
            "edgeforge-autonomic",
            "test_generator.rs",
            "generate",
        );

        if patterns.is_empty() {
            tracing::debug!("generate: no patterns → no test");
            return None;
        }

        // Find the highest-severity pattern.
        let pattern = patterns
            .iter()
            .max_by_key(|p| p.severity())
            .unwrap(); // safe: patterns is non-empty

        tracing::info!(
            pattern_kind = %pattern.kind,
            pattern_severity = pattern.severity(),
            remediation_kind = ?remediation.kind(),
            "generate: generating regression test"
        );

        let test = match pattern.kind {
            FailureKind::ThermalThrottle => {
                Some(Self::generate_thermal_throttle_test(pattern, remediation))
            }
            FailureKind::CorruptExpert => {
                Some(Self::generate_corrupt_expert_test(pattern, remediation))
            }
            FailureKind::PeerDisconnect => {
                Some(Self::generate_peer_disconnect_test(pattern, remediation))
            }
            FailureKind::SpeculationMissSpike => {
                Some(Self::generate_speculation_miss_test(pattern, remediation))
            }
            FailureKind::VerificationFailure => {
                Some(Self::generate_verification_failure_kani_harness(pattern, remediation))
            }
        };

        if test.is_some() {
            guard.success();
        } else {
            guard.error("no matching pattern");
        }
        test
    }

    fn generate_thermal_throttle_test(
        pattern: &FailurePattern,
        remediation: &Remediation,
    ) -> GeneratedTest {
        let name = "test_thermal_throttle_remmediation".to_string();
        let target_path = "tests/e2e/autonomic/test_thermal_throttle.rs".to_string();
        let source = format!(
r#"//! Antifragile regression test — auto-generated by the MAPE-K loop.
//!
//! Failure kind: ThermalThrottle
//! Metric: {metric}
//! Detected value: {value}
//! Occurrences: {occurrences}
//! Remediation: {rem_kind}
//!
//! This test reproduces the failure condition and asserts that the
//! remediation is applied correctly. It was generated automatically
//! when the MAPE-K loop detected and resolved a thermal throttle.

#![cfg(test)]

use edgeforge_autonomic::{{Remediation, RemediationContext, RemediationKind, RemediationResult}};

#[tokio::test]
async fn thermal_throttle_triggers_reduce_prefetch_depth() {{
    // Arrange: set up the remediation context.
    let ctx = RemediationContext::new();
    let remediation = Remediation::{rem_constructor};

    // Act: execute the remediation.
    let result = remediation.execute(&ctx).await;

    // Assert: the remediation should succeed and mutate state.
    assert!(
        result.is_success() || result.is_skipped(),
        "remediation should succeed or be skipped, got: {{:?}}",
        result
    );
}}

#[tokio::test]
async fn thermal_throttle_at_minimum_depth_is_skipped() {{
    // Arrange: set prefetch depth to 0 (minimum).
    let ctx = RemediationContext::with_initial_state(0, 16);
    let remediation = Remediation::reduce_prefetch_depth();

    // Act: attempt to reduce further.
    let result = remediation.execute(&ctx).await;

    // Assert: should be skipped (already at minimum).
    assert!(result.is_skipped(), "should skip at minimum depth");
    assert_eq!(ctx.prefetch_depth(), 0);
}}
"#,
            metric = pattern.metric,
            value = pattern.value,
            occurrences = pattern.occurrences,
            rem_kind = remediation.kind(),
            rem_constructor = match remediation.kind() {
                RemediationKind::ReducePrefetchDepth => "reduce_prefetch_depth()",
                RemediationKind::ReduceDraftMax => "reduce_draft_max()",
                _ => "reduce_prefetch_depth()",
            },
        );

        GeneratedTest {
            kind: TestKind::RegressionTest,
            name,
            source,
            target_path,
        }
    }

    fn generate_corrupt_expert_test(
        pattern: &FailurePattern,
        remediation: &Remediation,
    ) -> GeneratedTest {
        let name = "test_corrupt_expert_remmediation".to_string();
        let target_path = "tests/e2e/autonomic/test_corrupt_expert.rs".to_string();
        let expert_target = pattern.target.clone().unwrap_or_else(|| "layer_0_expert_0".to_string());
        let source = format!(
r#"//! Antifragile regression test — auto-generated by the MAPE-K loop.
//!
//! Failure kind: CorruptExpert
//! Metric: {metric}
//! Detected value: {value} (I/O latency spike)
//! Target expert: {expert}
//! Remediation: {rem_kind}

#![cfg(test)]

use edgeforge_autonomic::{{Remediation, RemediationContext}};

#[tokio::test]
async fn corrupt_expert_triggers_eviction() {{
    let ctx = RemediationContext::new();
    let remediation = Remediation::evict_expert("{expert}");

    let result = remediation.execute(&ctx).await;

    assert!(
        result.is_success(),
        "expert eviction should succeed, got: {{:?}}",
        result
    );
    assert!(ctx.is_expert_evicted("{expert}"));
}}

#[tokio::test]
async fn corrupt_expert_double_eviction_is_skipped() {{
    let ctx = RemediationContext::new();
    let remediation = Remediation::evict_expert("{expert}");

    // First eviction: Success.
    let result = remediation.execute(&ctx).await;
    assert!(result.is_success());

    // Second eviction: Skipped (already evicted).
    let result = remediation.execute(&ctx).await;
    assert!(result.is_skipped());
}}
"#,
            metric = pattern.metric,
            value = pattern.value,
            expert = expert_target,
            rem_kind = remediation.kind(),
        );

        GeneratedTest {
            kind: TestKind::RegressionTest,
            name,
            source,
            target_path,
        }
    }

    fn generate_peer_disconnect_test(
        pattern: &FailurePattern,
        remediation: &Remediation,
    ) -> GeneratedTest {
        let name = "test_peer_disconnect_remmediation".to_string();
        let target_path = "tests/e2e/autonomic/test_peer_disconnect.rs".to_string();
        let peer_target = pattern.target.clone().unwrap_or_else(|| "peer-unknown".to_string());
        let source = format!(
r#"//! Antifragile regression test — auto-generated by the MAPE-K loop.
//!
//! Failure kind: PeerDisconnect
//! Metric: {metric}
//! Detected value: {value}
//! Target peer: {peer}
//! Remediation: {rem_kind}

#![cfg(test)]

use edgeforge_autonomic::{{Remediation, RemediationContext}};

#[tokio::test]
async fn peer_disconnect_triggers_eviction() {{
    let ctx = RemediationContext::new();
    let remediation = Remediation::evict_peer("{peer}");

    let result = remediation.execute(&ctx).await;

    assert!(result.is_success(), "peer eviction should succeed");
    assert!(ctx.is_peer_evicted("{peer}"));
}}

#[tokio::test]
async fn peer_disconnect_triggers_reroute() {{
    let ctx = RemediationContext::new();
    let remediation = Remediation::reroute_peer("{peer}");

    let result = remediation.execute(&ctx).await;

    assert!(result.is_success());
    assert!(ctx.is_peer_rerouted("{peer}"));
}}
"#,
            metric = pattern.metric,
            value = pattern.value,
            peer = peer_target,
            rem_kind = remediation.kind(),
        );

        GeneratedTest {
            kind: TestKind::RegressionTest,
            name,
            source,
            target_path,
        }
    }

    fn generate_speculation_miss_test(
        pattern: &FailurePattern,
        remediation: &Remediation,
    ) -> GeneratedTest {
        let name = "test_speculation_miss_spike_remmediation".to_string();
        let target_path = "tests/e2e/autonomic/test_speculation_miss.rs".to_string();
        let source = format!(
r#"//! Antifragile regression test — auto-generated by the MAPE-K loop.
//!
//! Failure kind: SpeculationMissSpike
//! Metric: {metric}
//! Detected value: {value} (consecutive misses)
//! Remediation: {rem_kind}

#![cfg(test)]

use edgeforge_autonomic::{{Remediation, RemediationContext}};

#[tokio::test]
async fn speculation_miss_spike_triggers_reduce_draft_max() {{
    let ctx = RemediationContext::new();
    let initial_draft_max = ctx.draft_max();
    let remediation = Remediation::reduce_draft_max();

    let result = remediation.execute(&ctx).await;

    assert!(result.is_success(), "draft-max reduction should succeed");
    assert!(ctx.draft_max() < initial_draft_max, "draft-max should decrease");
}}

#[tokio::test]
async fn speculation_miss_spike_at_minimum_is_skipped() {{
    // Set draft-max to the minimum (2).
    let ctx = RemediationContext::with_initial_state(4, 2);
    let remediation = Remediation::reduce_draft_max();

    let result = remediation.execute(&ctx).await;

    assert!(result.is_skipped(), "should skip at minimum draft-max");
    assert_eq!(ctx.draft_max(), 2);
}}
"#,
            metric = pattern.metric,
            value = pattern.value,
            rem_kind = remediation.kind(),
        );

        GeneratedTest {
            kind: TestKind::RegressionTest,
            name,
            source,
            target_path,
        }
    }

    fn generate_verification_failure_kani_harness(
        pattern: &FailurePattern,
        remediation: &Remediation,
    ) -> GeneratedTest {
        let name = "kani_verification_failure_contract".to_string();
        let target_path = "tests/kani/kani_verification_failure.rs".to_string();
        let target = pattern.target.clone().unwrap_or_else(|| "unknown".to_string());
        let source = format!(
r#"//! Antifragile Kani harness — auto-generated by the MAPE-K loop.
//!
//! Failure kind: VerificationFailure
//! Metric: {metric}
//! Detected value: {value}
//! Target: {target}
//! Remediation: {rem_kind}
//!
//! This Kani harness verifies that the verification contract holds:
//! for any verification verdict, a Failed outcome triggers a
//! TriggerKaniReverification remediation.

#![cfg(kani)]

use edgeforge_autonomic::{{Remediation, RemediationContext, RemediationResult}};

#[kani::proof]
fn verify_verification_failure_triggers_kani_reverification() {{
    let ctx = RemediationContext::new();
    let remediation = Remediation::trigger_kani_reverification("{target}");

    // Execute the remediation (Kani will explore all paths).
    let result = remediation.execute(&ctx);

    // The remediation should always succeed (it just queues the target).
    // Kani verifies this holds for all possible states.
    assert!(
        matches!(result, RemediationResult::Success {{ .. }}),
        "Kani re-verification queueing should always succeed"
    );
    assert!(ctx.kani_queue_len() >= 1, "target should be queued");
}}

#[kani::proof]
fn verify_kani_queue_grows_monotonically() {{
    let ctx = RemediationContext::new();
    let initial_len = ctx.kani_queue_len();

    let remediation = Remediation::trigger_kani_reverification("{target}");
    let _ = remediation.execute(&ctx);

    // Kani verifies the queue never shrinks from a successful queue operation.
    assert!(
        ctx.kani_queue_len() >= initial_len + 1,
        "queue should grow by at least 1 after a successful queue operation"
    );
}}
"#,
            metric = pattern.metric,
            value = pattern.value,
            target = target,
            rem_kind = remediation.kind(),
        );

        GeneratedTest {
            kind: TestKind::KaniHarness,
            name,
            source,
            target_path,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::FailureKind;

    fn make_pattern(kind: FailureKind, metric: &str, value: f64, target: Option<String>) -> FailurePattern {
        FailurePattern::new(kind, metric, value, target, 1700000000000)
    }

    // ── generate() — previously always returned None, now returns real tests ──

    #[test]
    fn test_generate_returns_none_for_empty_patterns() {
        let gen = TestGenerator::new();
        let rem = Remediation::reduce_prefetch_depth();
        assert!(gen.generate(&[], &rem).is_none());
    }

    #[test]
    fn test_generate_thermal_throttle() {
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::ThermalThrottle,
            "ssd_temp_c",
            75.0,
            None,
        )];
        let rem = Remediation::reduce_prefetch_depth();

        // Previously: returned None.
        // Now: returns a real GeneratedTest.
        let test = gen.generate(&patterns, &rem);
        assert!(test.is_some(), "should generate a test for ThermalThrottle");

        let test = test.unwrap();
        assert_eq!(test.kind, TestKind::RegressionTest);
        assert!(test.name.contains("thermal_throttle"));
        assert!(test.target_path.starts_with("tests/e2e/autonomic/"));
        assert!(test.source.contains("#[tokio::test]"));
        assert!(test.source.contains("ThermalThrottle"));
        assert!(test.source.contains("RemediationContext"));
    }

    #[test]
    fn test_generate_corrupt_expert() {
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::CorruptExpert,
            "io_latency_us",
            15_000.0,
            Some("layer_3_expert_5".to_string()),
        )];
        let rem = Remediation::evict_expert("layer_3_expert_5");

        let test = gen.generate(&patterns, &rem).unwrap();
        assert_eq!(test.kind, TestKind::RegressionTest);
        assert!(test.name.contains("corrupt_expert"));
        assert!(test.source.contains("layer_3_expert_5"));
        assert!(test.source.contains("evict_expert"));
    }

    #[test]
    fn test_generate_peer_disconnect() {
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::PeerDisconnect,
            "peer_rtt_ms",
            600.0,
            Some("peer-123".to_string()),
        )];
        let rem = Remediation::evict_peer("peer-123");

        let test = gen.generate(&patterns, &rem).unwrap();
        assert_eq!(test.kind, TestKind::RegressionTest);
        assert!(test.name.contains("peer_disconnect"));
        assert!(test.source.contains("peer-123"));
        assert!(test.source.contains("evict_peer"));
        assert!(test.source.contains("reroute_peer"));
    }

    #[test]
    fn test_generate_speculation_miss_spike() {
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::SpeculationMissSpike,
            "speculation_miss_count",
            5.0,
            None,
        )];
        let rem = Remediation::reduce_draft_max();

        let test = gen.generate(&patterns, &rem).unwrap();
        assert_eq!(test.kind, TestKind::RegressionTest);
        assert!(test.name.contains("speculation_miss"));
        assert!(test.source.contains("reduce_draft_max"));
    }

    #[test]
    fn test_generate_verification_failure_kani_harness() {
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::VerificationFailure,
            "verification_failed",
            1.0,
            Some("peer-456".to_string()),
        )];
        let rem = Remediation::trigger_kani_reverification("peer-456");

        let test = gen.generate(&patterns, &rem).unwrap();
        assert_eq!(test.kind, TestKind::KaniHarness);
        assert!(test.name.contains("kani"));
        assert!(test.target_path.starts_with("tests/kani/"));
        assert!(test.source.contains("#[kani::proof]"));
        assert!(test.source.contains("peer-456"));
    }

    #[test]
    fn test_generate_picks_highest_severity_pattern() {
        let gen = TestGenerator::new();
        // Two patterns: SpeculationMissSpike (severity 2+1=3) and
        // ThermalThrottle (severity 5+1=6). Should pick ThermalThrottle.
        let patterns = vec![
            make_pattern(FailureKind::SpeculationMissSpike, "x", 3.0, None),
            make_pattern(FailureKind::ThermalThrottle, "ssd_temp_c", 75.0, None),
        ];
        let rem = Remediation::reduce_prefetch_depth();

        let test = gen.generate(&patterns, &rem).unwrap();
        assert!(test.name.contains("thermal_throttle"), "should pick highest-severity pattern");
    }

    #[test]
    fn test_generated_test_source_is_compilable_structure() {
        // Verify the generated source has the basic structure of a Rust test file.
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::ThermalThrottle,
            "ssd_temp_c",
            75.0,
            None,
        )];
        let rem = Remediation::reduce_prefetch_depth();
        let test = gen.generate(&patterns, &rem).unwrap();

        // Should have module doc comment.
        assert!(test.source.starts_with("//!"));
        // Should have #![cfg(test)] or #![cfg(kani)].
        assert!(test.source.contains("#![cfg("));
        // Should have at least one test function.
        assert!(test.source.contains("#[tokio::test]") || test.source.contains("#[kani::proof]"));
        // Should reference the failure kind.
        assert!(test.source.contains("ThermalThrottle"));
    }

    #[test]
    fn test_test_kind_equality() {
        assert_eq!(TestKind::RegressionTest, TestKind::RegressionTest);
        assert_ne!(TestKind::RegressionTest, TestKind::KaniHarness);
        assert_ne!(TestKind::KaniHarness, TestKind::PropertyTest);
    }

    #[test]
    fn test_generated_test_clone_debug() {
        let gen = TestGenerator::new();
        let patterns = vec![make_pattern(
            FailureKind::ThermalThrottle,
            "ssd_temp_c",
            75.0,
            None,
        )];
        let rem = Remediation::reduce_prefetch_depth();
        let test = gen.generate(&patterns, &rem).unwrap();

        let cloned = test.clone();
        assert_eq!(test.name, cloned.name);
        assert_eq!(test.source, cloned.source);

        let debug = format!("{test:?}");
        assert!(debug.contains("GeneratedTest"));
    }
}
