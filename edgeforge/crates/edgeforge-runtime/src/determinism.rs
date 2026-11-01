//! # Determinism Toggle (F-SCHED-05)
//!
//! ## Purpose
//! Reproducible mode for testing. Fixed seed + deterministic reductions.
//! Costs ~15% throughput.
//!
//! ## What This File Does
//! - `DeterminismConfig` — enabled flag + seed
//! - `set_determinism(config)` — sets the global determinism configuration
//!   (writes to `AtomicBool` + `AtomicU64`)
//! - `is_deterministic()` — returns whether determinism mode is enabled
//! - `global_seed()` — returns the global seed (if determinism is enabled)
//! - `effective_seed(params_seed)` — returns `params_seed` if `Some`, else
//!   falls back to the global seed if determinism is enabled, else `None`
//!
//! ## How It Works
//! The SsdEngine calls `effective_seed()` in its `generate()` method:
//! ```ignore
//! let seed = effective_seed(params.seed)
//!     .unwrap_or_else(|| fnv1a64(prompt_text.as_bytes()));
//! ```
//! When `set_determinism()` is called with `enabled: true`, the global seed
//! is stored in an `AtomicU64`. All subsequent `SsdEngine::generate()` calls
//! use this seed (unless `GenParams::seed` explicitly overrides it).
//!
//! ## Dependencies
//! - `serde` — for config serialization
//! - `std::sync::atomic` — for thread-safe global state

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Global determinism enabled flag.
static DETERMINISM_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global determinism seed (only meaningful when DETERMINISM_ENABLED is true).
static DETERMINISM_SEED: AtomicU64 = AtomicU64::new(0);

/// Determinism configuration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DeterminismConfig {
    /// Whether determinism mode is enabled.
    pub enabled: bool,
    /// The fixed random seed.
    pub seed: u64,
}

impl Default for DeterminismConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            seed: 0,
        }
    }
}

/// Set the global determinism configuration.
///
/// ## Purpose
/// When enabled, the inference engine uses the fixed seed for all random
/// operations (unless `GenParams::seed` explicitly overrides it).
///
/// ## Parameters
/// - `config`: The determinism config (enabled + seed)
///
/// ## Side Effects
/// - Sets `DETERMINISM_ENABLED` to `config.enabled`
/// - Sets `DETERMINISM_SEED` to `config.seed`
/// - When enabled, the inference engine will:
///   - Use the fixed seed for all random operations (unless overridden per-call)
///   - Accept ~15% throughput cost
///
/// ## Logging
/// Logs the configuration change via `tracing::info!`.
pub fn set_determinism(config: DeterminismConfig) {
    DETERMINISM_ENABLED.store(config.enabled, Ordering::SeqCst);
    DETERMINISM_SEED.store(config.seed, Ordering::SeqCst);

    tracing::info!(
        enabled = config.enabled,
        seed = config.seed,
        "set_determinism: global determinism configuration updated"
    );
}

/// Returns whether determinism mode is globally enabled.
///
/// ## Returns
/// `true` if `set_determinism()` was called with `enabled: true`.
#[must_use]
pub fn is_deterministic() -> bool {
    DETERMINISM_ENABLED.load(Ordering::SeqCst)
}

/// Returns the global determinism seed.
///
/// ## Returns
/// The seed set by `set_determinism()`. Returns 0 if determinism is disabled
/// (callers should check `is_deterministic()` first).
#[must_use]
pub fn global_seed() -> u64 {
    DETERMINISM_SEED.load(Ordering::SeqCst)
}

/// Compute the effective seed for a generation call.
///
/// ## Purpose
/// Resolves the seed priority:
/// 1. If `params_seed` is `Some(seed)`, use it (per-call override)
/// 2. Else if global determinism is enabled, use the global seed
/// 3. Else return `None` (the caller will derive a seed from the prompt)
///
/// ## Parameters
/// - `params_seed`: The per-call seed from `GenParams::seed`.
///
/// ## Returns
/// `Some(seed)` if a seed is available (per-call or global), `None` if the
/// caller should derive a seed from the prompt.
#[must_use]
pub fn effective_seed(params_seed: Option<u64>) -> Option<u64> {
    if let Some(seed) = params_seed {
        return Some(seed);
    }
    if is_deterministic() {
        Some(global_seed())
    } else {
        None
    }
}

/// Reset the determinism configuration to defaults (disabled, seed=0).
///
/// ## Purpose
/// Utility for tests to reset the global state between test cases.
pub fn reset_determinism() {
    DETERMINISM_ENABLED.store(false, Ordering::SeqCst);
    DETERMINISM_SEED.store(0, Ordering::SeqCst);
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DeterminismConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.seed, 0);
    }

    #[test]
    fn test_set_determinism_enabled() {
        reset_determinism();
        assert!(!is_deterministic());

        set_determinism(DeterminismConfig {
            enabled: true,
            seed: 42,
        });

        assert!(is_deterministic());
        assert_eq!(global_seed(), 42);

        reset_determinism();
    }

    #[test]
    fn test_set_determinism_disabled() {
        reset_determinism();

        set_determinism(DeterminismConfig {
            enabled: false,
            seed: 99,
        });

        assert!(!is_deterministic());
        // The seed is stored even when disabled (it just isn't used).
        assert_eq!(global_seed(), 99);

        reset_determinism();
    }

    #[test]
    fn test_effective_seed_per_call_override() {
        reset_determinism();

        // Even with determinism enabled, a per-call seed takes priority.
        set_determinism(DeterminismConfig {
            enabled: true,
            seed: 42,
        });

        let seed = effective_seed(Some(999));
        assert_eq!(seed, Some(999));

        reset_determinism();
    }

    #[test]
    fn test_effective_seed_falls_back_to_global() {
        reset_determinism();

        set_determinism(DeterminismConfig {
            enabled: true,
            seed: 42,
        });

        let seed = effective_seed(None);
        assert_eq!(seed, Some(42));

        reset_determinism();
    }

    #[test]
    fn test_effective_seed_returns_none_when_disabled() {
        reset_determinism();

        // Determinism disabled + no per-call seed → None.
        let seed = effective_seed(None);
        assert_eq!(seed, None);
    }

    #[test]
    fn test_effective_seed_per_call_when_disabled() {
        reset_determinism();

        // Determinism disabled but per-call seed is provided → use it.
        let seed = effective_seed(Some(123));
        assert_eq!(seed, Some(123));
    }

    #[test]
    fn test_reset_determinism() {
        set_determinism(DeterminismConfig {
            enabled: true,
            seed: 42,
        });
        assert!(is_deterministic());

        reset_determinism();
        assert!(!is_deterministic());
        assert_eq!(global_seed(), 0);
    }

    #[test]
    fn test_set_determinism_multiple_calls() {
        reset_determinism();

        set_determinism(DeterminismConfig {
            enabled: true,
            seed: 1,
        });
        assert_eq!(global_seed(), 1);

        set_determinism(DeterminismConfig {
            enabled: true,
            seed: 2,
        });
        assert_eq!(global_seed(), 2);

        set_determinism(DeterminismConfig {
            enabled: false,
            seed: 3,
        });
        assert!(!is_deterministic());

        reset_determinism();
    }
}
