// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
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
pub enum Precision {
    Fp32 = 0,
    Fp16 = 1,
    Bf16 = 2,
    Q8 = 3,
    Q4 = 4,
    Q3 = 5,
}
impl Precision {
    pub const fn bytes_per_param_exact(self) -> f32 {
        match self {
            Self::Fp32 => 4.0,
            Self::Fp16 | Self::Bf16 => 2.0,
            Self::Q8 => 1.0,
            Self::Q4 => 0.5,
            Self::Q3 => 0.375,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
/// `ModelArch` enumeration — see module documentation for variants.
pub enum ModelArch {
    Moe = 0,
    Dense = 1,
    Rwkv = 2,
    Mamba = 3,
}
#[derive(Clone, Copy, PartialEq, Eq)]
/// `LegacyError` enumeration — see module documentation for variants.
pub enum LegacyError {
    NotFound,
    InvalidArgument,
    Unauthorized,
    RateLimited,
    Internal,
    NotImplemented,
    Timeout,
    IoError,
    NetworkError,
    SecurityError,
    VerificationFailed,
}
impl LegacyError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "EF_NOT_FOUND",
            Self::InvalidArgument => "EF_INVALID_ARGUMENT",
            Self::Unauthorized => "EF_UNAUTHORIZED",
            Self::RateLimited => "EF_RATE_LIMITED",
            Self::Internal => "EF_INTERNAL",
            Self::NotImplemented => "EF_NOT_IMPLEMENTED",
            Self::Timeout => "EF_TIMEOUT",
            Self::IoError => "EF_IO_ERROR",
            Self::NetworkError => "EF_NETWORK_ERROR",
            Self::SecurityError => "EF_SECURITY_ERROR",
            Self::VerificationFailed => "EF_VERIFICATION_FAILED",
        }
    }
}
/// The `ComputeSubstrate` trait — see module documentation for the contract.
pub trait ComputeSubstrate: Send + Sync {
    type ExecutionContext;
    type Program;
    type Output;
    fn execute(
        &self,
        program: &Self::Program,
        ctx: &mut Self::ExecutionContext,
    ) -> Result<Self::Output, ComputeError>;
    fn capabilities(&self) -> SubstrateCapabilities;
}
#[derive(Clone, Copy, PartialEq, Eq)]
/// `SubstrateCapabilities` — see module documentation for purpose and usage.
pub struct SubstrateCapabilities {
    pub classical: bool,
    pub quantum: bool,
    pub neuromorphic: bool,
    pub neural_interface: bool,
}
impl SubstrateCapabilities {
    pub const fn classical_only() -> Self {
        Self {
            classical: true,
            quantum: false,
            neuromorphic: false,
            neural_interface: false,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
/// `ComputeError` enumeration — see module documentation for variants.
pub enum ComputeError {
    SubstrateUnsupported,
    ProgramMalformed,
    ExecutionFailed,
    Timeout,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precision_monotonic() {
        assert!(Precision::Fp32.bytes_per_param_exact() >= Precision::Fp16.bytes_per_param_exact());
    }
    #[test]
    fn error_codes_stable() {
        assert_eq!(LegacyError::NotFound.code(), "EF_NOT_FOUND");
    }
}
