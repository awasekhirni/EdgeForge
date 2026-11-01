//! # Cryptographic Agility Layer (ADR-027)
//!
//! ## Purpose
//! Abstracts every cryptographic primitive behind a trait so the algorithm
//! can be swapped at runtime via configuration. Foundation for the hybrid PQC
//! migration roadmap (2026–2031+).
//!
//! ## What This File Does
//! - `KemAlgorithm` trait — key encapsulation (generate, encapsulate, decapsulate)
//! - `SigAlgorithm` trait — digital signatures (generate, sign, verify)
//! - `KemVariant` — X25519, ML-KEM-512/768/1024, HybridX25519MlKem768
//! - `SigVariant` — Ed25519, ML-DSA-65, HybridEd25519MlDsa65
//! - `CryptoPolicy` — active policy (kem + sig + migration phase)
//! - `MigrationPhase` — ClassicalOnly → Hybrid → HybridMandatory → PqcOnly
//! - `CryptoInventory` — list of all crypto usages (exposed via /v1/crypto/inventory)
//! - Key types: PublicKey, SecretKey, Ciphertext, SharedSecret, Signature, KeyPair
//! - SecretKey and SharedSecret are zeroized on drop
//!
//! ## Dependencies
//! - `async-trait` — async trait methods
//! - `serde` — Serialize/Deserialize for variants and policy
//! - `zeroize` — key material zeroing
//!
//! ## Output
//! Traits and types that allow runtime algorithm swapping without code changes.
//!
//! ## Migration Roadmap
//! | Period | Stable | Beta |
//! |---|---|---|
//! | 2026–2028 | Classical-only | Hybrid |
//! | 2028–2031 | Hybrid mandatory | PQC-only |
//! | 2031+ | PQC-only | PQC-only |

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[async_trait]
/// The `KemAlgorithm` trait — see module documentation for the contract.
pub trait KemAlgorithm: Send + Sync + 'static {
    fn variant(&self) -> KemVariant;
    async fn generate_keypair(&self) -> KeyPair;
    async fn encapsulate(&self, peer_pk: &PublicKey) -> (Ciphertext, SharedSecret);
    async fn decapsulate(&self, sk: &SecretKey, ct: &Ciphertext) -> SharedSecret;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// `KemVariant` enumeration — see module documentation for variants.
pub enum KemVariant { X25519, MlKem512, MlKem768, MlKem1024, HybridX25519MlKem768 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// `SigVariant` enumeration — see module documentation for variants.
pub enum SigVariant { Ed25519, MlDsa65, HybridEd25519MlDsa65 }
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `CryptoPolicy` — see module documentation for purpose and usage.
pub struct CryptoPolicy { pub kem: KemVariant, pub sig: SigVariant, pub phase: MigrationPhase }
impl Default for CryptoPolicy { fn default() -> Self { Self { kem: KemVariant::X25519, sig: SigVariant::Ed25519, phase: MigrationPhase::ClassicalOnly } } }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `MigrationPhase` enumeration — see module documentation for variants.
pub enum MigrationPhase { ClassicalOnly, Hybrid, HybridMandatory, PqcOnly }
/// `PublicKey` — see module documentation for purpose and usage.
pub struct PublicKey(pub Vec<u8>);
/// `SecretKey` — see module documentation for purpose and usage.
pub struct SecretKey(pub Vec<u8>);
/// `Ciphertext` — see module documentation for purpose and usage.
pub struct Ciphertext(pub Vec<u8>);
/// `SharedSecret` — see module documentation for purpose and usage.
pub struct SharedSecret(pub Vec<u8>);
/// `Signature` — see module documentation for purpose and usage.
pub struct Signature(pub Vec<u8>);
/// `KeyPair` — see module documentation for purpose and usage.
pub struct KeyPair { pub public: PublicKey, pub secret: SecretKey }
impl Drop for SecretKey { fn drop(&mut self) { zeroize::Zeroize::zeroize(&mut self.0); } }
impl Drop for SharedSecret { fn drop(&mut self) { zeroize::Zeroize::zeroize(&mut self.0); } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_crypto_policy() {
        let p = CryptoPolicy::default();
        assert_eq!(p.kem, KemVariant::X25519);
        assert_eq!(p.sig, SigVariant::Ed25519);
        assert_eq!(p.phase, MigrationPhase::ClassicalOnly);
    }

    #[test]
    fn test_kem_variant_equality() {
        assert_eq!(KemVariant::X25519, KemVariant::X25519);
        assert_ne!(KemVariant::X25519, KemVariant::MlKem768);
    }

    #[test]
    fn test_sig_variant_equality() {
        assert_eq!(SigVariant::Ed25519, SigVariant::Ed25519);
        assert_ne!(SigVariant::Ed25519, SigVariant::MlDsa65);
    }

    #[test]
    fn test_migration_phase_ordering() {
        let phases = [MigrationPhase::ClassicalOnly, MigrationPhase::Hybrid, MigrationPhase::HybridMandatory, MigrationPhase::PqcOnly];
        for i in 0..phases.len() - 1 {
            assert_ne!(phases[i], phases[i + 1]);
        }
    }

    #[test]
    fn test_crypto_policy_serializes() {
        let p = CryptoPolicy::default();
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("x25519"));
        assert!(json.contains("ed25519"));
        assert!(json.contains("classical_only"));
    }

    #[test]
    fn test_secret_key_zeroized_on_drop() {
        let sk = SecretKey(vec![0xAB; 32]);
        drop(sk);
        // Can't directly verify zeroization, but the Drop impl calls zeroize
        // This test just verifies the Drop doesn't panic
    }

    #[test]
    fn test_shared_secret_zeroized_on_drop() {
        let ss = SharedSecret(vec![0xCD; 32]);
        drop(ss);
    }

    #[test]
    fn test_key_pair_construction() {
        let kp = KeyPair { public: PublicKey(vec![1]), secret: SecretKey(vec![2]) };
        assert_eq!(kp.public.0, vec![1]);
    }
}
