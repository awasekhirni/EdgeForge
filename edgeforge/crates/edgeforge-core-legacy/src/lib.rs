//! # edgeforge-core-legacy — Zero-Dependency Core (ADR-024)
//!
//! ## Purpose
//! The survivable core of EdgeForge. Depends on **nothing** and is
//! `#![forbid(unsafe_code)]`. If the rest of EdgeForge is ever rewritten in
//! another language, this crate's specification is the authoritative source
//! for the data model and error taxonomy.
//!
//! ## What This File Does
//! - `ModelId`, `JobId`, `NodeId` — 128-bit identifiers
//! - `Precision` — precision tier enum with `bytes_per_param_exact()`
//! - `ModelArch` — architecture family (MoE/Dense/RWKV/Mamba)
//! - `LegacyError` — error taxonomy with stable string codes (EF_NOT_FOUND, etc.)
//! - `ComputeSubstrate` trait — extension point for quantum/neuromorphic (ADR-031)
//! - `SubstrateCapabilities` — what a substrate can do
//!
//! ## Dependencies
//! **None.** Zero external dependencies. `#![no_std]`-compatible.
//!
//! ## Output
//! Reimplementable data model + error taxonomy + compute substrate trait.
//!
//! ## Contract
//! - `Precision::bytes_per_param_exact()` is monotonic (higher precision = more bytes)
//! - `LegacyError::code()` returns a stable string that never changes across versions
//! - `ComputeSubstrate::execute()` is idempotent for the same (program, ctx) pair

#![forbid(unsafe_code)]
//! Zero-dependency, no_std-compatible core for 50-year survivability (ADR-024).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// `ModelId` — see module documentation for purpose and usage.
pub struct ModelId(pub u128);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// `JobId` — see module documentation for purpose and usage.
pub struct JobId(pub u128);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// `NodeId` — see module documentation for purpose and usage.
pub struct NodeId(pub u128);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
/// `Precision` enumeration — see module documentation for variants.
pub enum Precision { Fp32 = 0, Fp16 = 1, Bf16 = 2, Q8 = 3, Q4 = 4, Q3 = 5 }
impl Precision {
    pub const fn bytes_per_param_exact(self) -> f32 {
        match self { Self::Fp32 => 4.0, Self::Fp16 | Self::Bf16 => 2.0, Self::Q8 => 1.0, Self::Q4 => 0.5, Self::Q3 => 0.375 }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
/// `ModelArch` enumeration — see module documentation for variants.
pub enum ModelArch { Moe = 0, Dense = 1, Rwkv = 2, Mamba = 3 }
#[derive(Clone, Copy, PartialEq, Eq)]
/// `LegacyError` enumeration — see module documentation for variants.
pub enum LegacyError { NotFound, InvalidArgument, Unauthorized, RateLimited, Internal, NotImplemented, Timeout, IoError, NetworkError, SecurityError, VerificationFailed }
impl LegacyError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "EF_NOT_FOUND", Self::InvalidArgument => "EF_INVALID_ARGUMENT",
            Self::Unauthorized => "EF_UNAUTHORIZED", Self::RateLimited => "EF_RATE_LIMITED",
            Self::Internal => "EF_INTERNAL", Self::NotImplemented => "EF_NOT_IMPLEMENTED",
            Self::Timeout => "EF_TIMEOUT", Self::IoError => "EF_IO_ERROR",
            Self::NetworkError => "EF_NETWORK_ERROR", Self::SecurityError => "EF_SECURITY_ERROR",
            Self::VerificationFailed => "EF_VERIFICATION_FAILED",
        }
    }
}
/// The `ComputeSubstrate` trait — see module documentation for the contract.
pub trait ComputeSubstrate: Send + Sync {
    type ExecutionContext; type Program; type Output;
    fn execute(&self, program: &Self::Program, ctx: &mut Self::ExecutionContext) -> Result<Self::Output, ComputeError>;
    fn capabilities(&self) -> SubstrateCapabilities;
}
#[derive(Clone, Copy, PartialEq, Eq)]
/// `SubstrateCapabilities` — see module documentation for purpose and usage.
pub struct SubstrateCapabilities { pub classical: bool, pub quantum: bool, pub neuromorphic: bool, pub neural_interface: bool }
impl SubstrateCapabilities { pub const fn classical_only() -> Self { Self { classical: true, quantum: false, neuromorphic: false, neural_interface: false } } }
#[derive(Clone, Copy, PartialEq, Eq)]
/// `ComputeError` enumeration — see module documentation for variants.
pub enum ComputeError { SubstrateUnsupported, ProgramMalformed, ExecutionFailed, Timeout }
#[cfg(test)]
mod tests {
    use super::*;

    // ── Precision tests ──

    #[test]
    fn precision_monotonic() {
        assert!(Precision::Fp32.bytes_per_param_exact() >= Precision::Fp16.bytes_per_param_exact());
        assert!(Precision::Fp16.bytes_per_param_exact() >= Precision::Q8.bytes_per_param_exact());
        assert!(Precision::Q8.bytes_per_param_exact() >= Precision::Q4.bytes_per_param_exact());
        assert!(Precision::Q4.bytes_per_param_exact() >= Precision::Q3.bytes_per_param_exact());
    }

    #[test]
    fn precision_fp32_is_4_bytes() {
        assert_eq!(Precision::Fp32.bytes_per_param_exact(), 4.0);
    }

    #[test]
    fn precision_fp16_bf16_are_2_bytes() {
        assert_eq!(Precision::Fp16.bytes_per_param_exact(), 2.0);
        assert_eq!(Precision::Bf16.bytes_per_param_exact(), 2.0);
    }

    #[test]
    fn precision_q8_is_1_byte() {
        assert_eq!(Precision::Q8.bytes_per_param_exact(), 1.0);
    }

    #[test]
    fn precision_q4_is_half_byte() {
        assert_eq!(Precision::Q4.bytes_per_param_exact(), 0.5);
    }

    #[test]
    fn precision_q3_is_3_8_byte() {
        assert_eq!(Precision::Q3.bytes_per_param_exact(), 0.375);
    }

    #[test]
    fn precision_equality() {
        assert_eq!(Precision::Fp32, Precision::Fp32);
        assert_ne!(Precision::Fp32, Precision::Fp16);
    }

    // ── LegacyError tests ──

    #[test]
    fn error_codes_stable() {
        assert_eq!(LegacyError::NotFound.code(), "EF_NOT_FOUND");
    }

    #[test]
    fn all_error_codes() {
        assert_eq!(LegacyError::NotFound.code(), "EF_NOT_FOUND");
        assert_eq!(LegacyError::InvalidArgument.code(), "EF_INVALID_ARGUMENT");
        assert_eq!(LegacyError::Unauthorized.code(), "EF_UNAUTHORIZED");
        assert_eq!(LegacyError::RateLimited.code(), "EF_RATE_LIMITED");
        assert_eq!(LegacyError::Internal.code(), "EF_INTERNAL");
        assert_eq!(LegacyError::NotImplemented.code(), "EF_NOT_IMPLEMENTED");
        assert_eq!(LegacyError::Timeout.code(), "EF_TIMEOUT");
        assert_eq!(LegacyError::IoError.code(), "EF_IO_ERROR");
        assert_eq!(LegacyError::NetworkError.code(), "EF_NETWORK_ERROR");
        assert_eq!(LegacyError::SecurityError.code(), "EF_SECURITY_ERROR");
        assert_eq!(LegacyError::VerificationFailed.code(), "EF_VERIFICATION_FAILED");
    }

    #[test]
    fn error_codes_are_unique() {
        let codes = [
            LegacyError::NotFound.code(),
            LegacyError::InvalidArgument.code(),
            LegacyError::Unauthorized.code(),
            LegacyError::RateLimited.code(),
            LegacyError::Internal.code(),
            LegacyError::NotImplemented.code(),
            LegacyError::Timeout.code(),
            LegacyError::IoError.code(),
            LegacyError::NetworkError.code(),
            LegacyError::SecurityError.code(),
            LegacyError::VerificationFailed.code(),
        ];
        let unique: std::collections::HashSet<&str> = codes.iter().copied().collect();
        assert_eq!(unique.len(), codes.len(), "all error codes must be unique");
    }

    #[test]
    fn error_codes_start_with_ef_prefix() {
        let all_errors = [
            LegacyError::NotFound, LegacyError::InvalidArgument,
            LegacyError::Unauthorized, LegacyError::RateLimited,
            LegacyError::Internal, LegacyError::NotImplemented,
            LegacyError::Timeout, LegacyError::IoError,
            LegacyError::NetworkError, LegacyError::SecurityError,
            LegacyError::VerificationFailed,
        ];
        for e in all_errors {
            assert!(e.code().starts_with("EF_"), "code {} should start with EF_", e.code());
        }
    }

    #[test]
    fn error_equality() {
        assert_eq!(LegacyError::NotFound, LegacyError::NotFound);
        assert_ne!(LegacyError::NotFound, LegacyError::Internal);
    }

    // ── ID type tests ──

    #[test]
    fn model_id_new() {
        let a = ModelId(1);
        let b = ModelId(2);
        assert_ne!(a, b);
    }

    #[test]
    fn job_id_new() {
        let a = JobId(1);
        let b = JobId(2);
        assert_ne!(a, b);
    }

    #[test]
    fn node_id_new() {
        let a = NodeId(1);
        let b = NodeId(2);
        assert_ne!(a, b);
    }

    #[test]
    fn id_ordering() {
        let a = ModelId(1);
        let b = ModelId(2);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn id_hash_equal() {
        let a = ModelId(42);
        let b = ModelId(42);
        let mut ha = std::collections::hash_map::DefaultHasher::new();
        let mut hb = std::collections::hash_map::DefaultHasher::new();
        use std::hash::Hash;
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    // ── ModelArch tests ──

    #[test]
    fn model_arch_equality() {
        assert_eq!(ModelArch::Moe, ModelArch::Moe);
        assert_ne!(ModelArch::Moe, ModelArch::Dense);
        assert_ne!(ModelArch::Dense, ModelArch::Rwkv);
        assert_ne!(ModelArch::Rwkv, ModelArch::Mamba);
    }

    #[test]
    fn model_arch_repr() {
        assert_eq!(ModelArch::Moe as u8, 0);
        assert_eq!(ModelArch::Dense as u8, 1);
        assert_eq!(ModelArch::Rwkv as u8, 2);
        assert_eq!(ModelArch::Mamba as u8, 3);
    }

    // ── SubstrateCapabilities tests ──

    #[test]
    fn classical_only_capabilities() {
        let cap = SubstrateCapabilities::classical_only();
        assert!(cap.classical);
        assert!(!cap.quantum);
        assert!(!cap.neuromorphic);
        assert!(!cap.neural_interface);
    }

    #[test]
    fn capabilities_equality() {
        let a = SubstrateCapabilities::classical_only();
        let b = SubstrateCapabilities::classical_only();
        assert_eq!(a, b);
    }

    // ── ComputeError tests ──

    #[test]
    fn compute_error_equality() {
        assert_eq!(ComputeError::SubstrateUnsupported, ComputeError::SubstrateUnsupported);
        assert_ne!(ComputeError::SubstrateUnsupported, ComputeError::ProgramMalformed);
        assert_ne!(ComputeError::ProgramMalformed, ComputeError::ExecutionFailed);
        assert_ne!(ComputeError::ExecutionFailed, ComputeError::Timeout);
    }
}
