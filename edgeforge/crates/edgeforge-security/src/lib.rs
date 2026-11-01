//! # edgeforge-security — L1 Security
//!
//! ## Purpose
//! Provides manifest verification (ed25519 signatures + SHA-256 integrity),
//! sandbox policy enforcement, crypto-agility (PQC traits), and at-rest
//! encryption hooks.
//!
//! ## Modules
//! - `manifest` — F-LOAD-04, F-SEC-05 signed manifest verification
//! - `hash` — SHA-256 streaming integrity check
//! - `sandbox` — F-SEC-03 platform sandbox policy
//! - `crypto_agility` — ADR-027 PQC traits (KemAlgorithm, SigAlgorithm)
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, ModelId, Precision
//! - `ring` — SHA-256, secure random
//! - `ed25519-dalek` — Ed25519 signature verification
//! - `zeroize` — key material zeroing
//! - `base64`, `hex` — encoding

pub mod crypto_agility;
pub mod hash;
pub mod manifest;
pub mod sandbox;
pub use manifest::{DefaultManifestVerifier, LicenseVerdict, ManifestVerifier, ModelArch, ModelManifest, UseCase};
pub use sandbox::{Sandbox, SandboxPolicy};
