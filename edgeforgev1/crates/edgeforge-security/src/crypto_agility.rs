// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[async_trait]
pub trait KemAlgorithm: Send + Sync + 'static {
    fn variant(&self) -> KemVariant;
    async fn generate_keypair(&self) -> KeyPair;
    async fn encapsulate(&self, peer_pk: &PublicKey) -> (Ciphertext, SharedSecret);
    async fn decapsulate(&self, sk: &SecretKey, ct: &Ciphertext) -> SharedSecret;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KemVariant {
    X25519,
    MlKem512,
    MlKem768,
    MlKem1024,
    HybridX25519MlKem768,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SigVariant {
    Ed25519,
    MlDsa65,
    HybridEd25519MlDsa65,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CryptoPolicy {
    pub kem: KemVariant,
    pub sig: SigVariant,
    pub phase: MigrationPhase,
}
impl Default for CryptoPolicy {
    fn default() -> Self {
        Self {
            kem: KemVariant::X25519,
            sig: SigVariant::Ed25519,
            phase: MigrationPhase::ClassicalOnly,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    ClassicalOnly,
    Hybrid,
    HybridMandatory,
    PqcOnly,
}
pub struct PublicKey(pub Vec<u8>);
pub struct SecretKey(pub Vec<u8>);
pub struct Ciphertext(pub Vec<u8>);
pub struct SharedSecret(pub Vec<u8>);
pub struct Signature(pub Vec<u8>);
pub struct KeyPair {
    pub public: PublicKey,
    pub secret: SecretKey,
}
impl Drop for SecretKey {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}
impl Drop for SharedSecret {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}
