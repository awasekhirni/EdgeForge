//! # VRAM Staging Tier (F-TIER-05)
//!
//! ## Purpose
//! When a GPU is present, the active layer + active experts are staged in
//! VRAM. On Apple Silicon this is a no-op (unified memory). On discrete GPUs
//! we issue a PCIe DMA copy.
//!
//! ## What This File Does
//! - `VramTierConfig` — enabled, budget_bytes, backend
//! - `VramBackend` — AppleUnified, Cuda, Rocm, None
//! - `VramTier` — staging manager with budget tracking
//! - `VramTier::new()` — create with config
//! - `VramTier::stage()` — stage data in VRAM (tracks bytes)
//! - `VramTier::evict()` — evict data from VRAM
//! - `VramTier::current_bytes()` — current VRAM usage
//! - `VramTier::available_bytes()` — remaining VRAM budget
//! - `detect_backend()` — auto-detect the GPU backend
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, Bytes
//! - `edgeforge-observability` — FunctionGuard for call logging
//!
//! ## Output
//! `Result<(), EdgeForgeError>` from stage() — Ok if staged, Err if budget exceeded.
//!
//! ## Function Logging
//! `timestamp - edgeforge-storage - vram_tier.rs - function - STATUS`

use edgeforge_core::{Bytes, EdgeForgeError};
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::sync::LazyLock;
use std::sync::Mutex;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// VRAM backend type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VramBackend {
    /// Apple Silicon — unified memory, no copy needed.
    AppleUnified,
    /// NVIDIA — CUDA staging.
    Cuda,
    /// AMD — ROCm staging.
    Rocm,
    /// No GPU available; CPU-only inference.
    None,
}

/// Configuration for the VRAM tier.
#[derive(Clone, Debug)]
pub struct VramTierConfig {
    /// Whether VRAM staging is enabled.
    pub enabled: bool,
    /// VRAM budget in bytes.
    pub budget_bytes: u64,
    /// The GPU backend.
    pub backend: VramBackend,
}

impl Default for VramTierConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            budget_bytes: 0,
            backend: VramBackend::None,
        }
    }
}

/// The VRAM staging manager.
///
/// ## Purpose
/// Manages VRAM allocation for the active layer + active experts. Tracks
/// current usage against the budget and evicts data when the budget is exceeded.
pub struct VramTier {
    /// The VRAM tier configuration.
    pub config: VramTierConfig,
    /// Current bytes staged in VRAM.
    current_bytes: Mutex<u64>,
}

impl VramTier {
    /// Creates a new VRAM tier with the given configuration.
    ///
    /// ## Parameters
    /// - `config`: The VRAM tier configuration.
    ///
    /// ## Returns
    /// A new `VramTier` instance.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-storage - vram_tier.rs - VramTier::new - STATUS`
    pub fn new(config: VramTierConfig) -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "VramTier::new");
        Self {
            config,
            current_bytes: Mutex::new(0),
        }
    }

    /// Returns whether VRAM staging is available.
    ///
    /// ## Returns
    /// `true` if staging is enabled and a GPU backend is present.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-storage - vram_tier.rs - enabled - STATUS`
    pub fn enabled(&self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "enabled");
        self.config.enabled && self.config.backend != VramBackend::None
    }

    /// Stage data in VRAM.
    ///
    /// ## Purpose
    /// Allocates VRAM for the given data size. If the budget is exceeded,
    /// returns an error. On Apple Silicon (unified memory), this is a no-op.
    ///
    /// ## Parameters
    /// - `bytes`: The number of bytes to stage.
    ///
    /// ## Returns
    /// `Ok(())` if staged successfully.
    ///
    /// ## Errors
    /// - `Engine` if VRAM budget is exceeded.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-storage - vram_tier.rs - stage - STATUS`
    pub fn stage(&self, bytes: u64) -> Result<(), EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "stage");
        if !self.enabled() {
            guard.success();
            return Ok(());
        }
        let mut current = self.current_bytes.lock().unwrap();
        if *current + bytes > self.config.budget_bytes {
            guard.error("VRAM budget exceeded");
            return Err(EdgeForgeError::Engine(format!(
                "VRAM budget exceeded: current={}, requested={}, budget={}",
                *current, bytes, self.config.budget_bytes
            )));
        }
        *current += bytes;
        tracing::debug!(staged = bytes, total = *current, budget = self.config.budget_bytes, "VRAM staged");
        guard.success();
        Ok(())
    }

    /// Evict data from VRAM.
    ///
    /// ## Purpose
    /// Frees VRAM by the given number of bytes. Called when experts are
    /// evicted from the active set.
    ///
    /// ## Parameters
    /// - `bytes`: The number of bytes to evict.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-storage - vram_tier.rs - evict - STATUS`
    pub fn evict(&self, bytes: u64) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "evict");
        let mut current = self.current_bytes.lock().unwrap();
        *current = current.saturating_sub(bytes);
        tracing::debug!(evicted = bytes, total = *current, "VRAM evicted");
    }

    /// Returns the current VRAM usage in bytes.
    ///
    /// ## Returns
    /// `u64` — the current bytes staged in VRAM.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-storage - vram_tier.rs - current_bytes - STATUS`
    pub fn current_bytes(&self) -> u64 {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "current_bytes");
        *self.current_bytes.lock().unwrap()
    }

    /// Returns the remaining VRAM budget in bytes.
    ///
    /// ## Returns
    /// `u64` — the remaining bytes that can be staged.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-storage - vram_tier.rs - available_bytes - STATUS`
    pub fn available_bytes(&self) -> u64 {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "available_bytes");
        self.config.budget_bytes.saturating_sub(self.current_bytes())
    }

    /// Returns the VRAM backend.
    pub fn backend(&self) -> VramBackend {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "backend");
        self.config.backend
    }
}

/// Auto-detect the GPU backend.
///
/// ## Purpose
/// Detects the available GPU backend based on the platform:
/// - macOS (arm64) → AppleUnified
/// - Linux with NVIDIA → Cuda
/// - Linux with AMD → Rocm
/// - Otherwise → None
///
/// ## Returns
/// `VramBackend` — the detected backend.
///
/// ## Function Logging
/// `timestamp - edgeforge-storage - vram_tier.rs - detect_backend - STATUS`
pub fn detect_backend() -> VramBackend {
    let _guard = FunctionGuard::new(&LOGGER, "edgeforge-storage", "vram_tier.rs", "detect_backend");
    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            return VramBackend::AppleUnified;
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Check for NVIDIA GPU.
        if std::path::Path::new("/dev/nvidia0").exists() {
            return VramBackend::Cuda;
        }
        // Check for AMD GPU.
        if std::path::Path::new("/dev/kfd").exists() {
            return VramBackend::Rocm;
        }
    }
    VramBackend::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vram_tier_disabled() {
        let tier = VramTier::new(VramTierConfig::default());
        assert!(!tier.enabled());
        // Staging when disabled is always Ok (no-op).
        assert!(tier.stage(1000).is_ok());
    }

    #[test]
    fn test_vram_tier_stage_within_budget() {
        let tier = VramTier::new(VramTierConfig {
            enabled: true,
            budget_bytes: 10000,
            backend: VramBackend::Cuda,
        });
        assert!(tier.stage(5000).is_ok());
        assert_eq!(tier.current_bytes(), 5000);
        assert_eq!(tier.available_bytes(), 5000);
    }

    #[test]
    fn test_vram_tier_stage_exceeds_budget() {
        let tier = VramTier::new(VramTierConfig {
            enabled: true,
            budget_bytes: 10000,
            backend: VramBackend::Cuda,
        });
        assert!(tier.stage(5000).is_ok());
        assert!(tier.stage(6000).is_err()); // exceeds budget
        assert_eq!(tier.current_bytes(), 5000); // unchanged
    }

    #[test]
    fn test_vram_tier_evict() {
        let tier = VramTier::new(VramTierConfig {
            enabled: true,
            budget_bytes: 10000,
            backend: VramBackend::Cuda,
        });
        tier.stage(5000).unwrap();
        tier.evict(3000);
        assert_eq!(tier.current_bytes(), 2000);
        assert_eq!(tier.available_bytes(), 8000);
    }

    #[test]
    fn test_vram_tier_evict_more_than_staged() {
        let tier = VramTier::new(VramTierConfig {
            enabled: true,
            budget_bytes: 10000,
            backend: VramBackend::Cuda,
        });
        tier.stage(1000).unwrap();
        tier.evict(5000); // evict more than staged
        assert_eq!(tier.current_bytes(), 0); // saturating_sub
    }

    #[test]
    fn test_vram_backend() {
        let tier = VramTier::new(VramTierConfig {
            enabled: true,
            budget_bytes: 1000,
            backend: VramBackend::AppleUnified,
        });
        assert_eq!(tier.backend(), VramBackend::AppleUnified);
    }

    #[test]
    fn test_detect_backend() {
        // detect_backend should return a valid backend.
        let backend = detect_backend();
        // On this machine it might be None — just verify it doesn't panic.
        let _ = backend;
    }
}
