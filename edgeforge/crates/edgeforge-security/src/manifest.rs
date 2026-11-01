//! # Signed Model Manifest Verification (F-LOAD-04, F-SEC-05)
//!
//! ## Purpose
//! Verifies that a model's manifest is signed by a trusted publisher and that
//! the on-disk weight file matches the manifest's SHA-256 hash. Also checks
//! the license against the user's intended use case.
//!
//! ## What This File Does
//! - `ModelManifest` — the manifest struct (schema v1)
//! - `ManifestVerifier` trait — verify_signature, verify_integrity, check_license
//! - `DefaultManifestVerifier` — production impl using ed25519-dalek
//! - `LicenseVerdict` — Permitted / Forbidden / UnsignedRequiresAcknowledgement
//! - `UseCase` — Personal / Research / Commercial
//! - `ModelArch` — MoE / Dense / RWKV / Mamba
//!
//! ## Dependencies
//! - `ed25519-dalek` — Ed25519 signature generation and verification
//! - `ring::digest` — SHA-256 streaming hash
//! - `base64` — signature encoding
//! - `hex` — hash encoding
//! - `serde_json` — canonical JSON for signed message
//!
//! ## Output
//! `Result<(), EdgeForgeError>` — Ok if manifest is valid, Err otherwise.
//!
//! ## Function: DefaultManifestVerifier::verify_signature
//! - **Purpose:** Verify the publisher's Ed25519 signature over the manifest
//! - **Parameters:** `&ModelManifest` — the manifest to verify
//! - **Returns:** `Result<(), EdgeForgeError>` — Ok if signature valid
//! - **Errors:** ManifestSignatureInvalid if signature is missing, key untrusted, or verify fails
//! - **Dependencies:** Calls `signed_message()` to canonicalize, then `VerifyingKey::verify()`
//!
//! ## Function: DefaultManifestVerifier::check_license
//! - **Purpose:** Check if the manifest's license permits the user's use case
//! - **Parameters:** `&ModelManifest`, `UseCase`
//! - **Returns:** `LicenseVerdict` — Permitted, Forbidden, or UnsignedRequiresAcknowledgement
//! - **Dependencies:** Checks `manifest.signature.is_none()` and `manifest.acceptable_uses`

use async_trait::async_trait;
use edgeforge_core::{EdgeForgeError, ModelId, Precision};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `UseCase` enumeration — see module documentation for variants.
pub enum UseCase { Personal, Research, Commercial }
#[derive(Clone, Debug, PartialEq, Eq)]
/// `LicenseVerdict` enumeration — see module documentation for variants.
pub enum LicenseVerdict { Permitted, Forbidden(String), UnsignedRequiresAcknowledgement }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `ModelArch` enumeration — see module documentation for variants.
pub enum ModelArch { Moe, Dense, Rwkv, Mamba }
#[derive(Clone, Debug, Serialize, Deserialize)]
/// `ModelManifest` — see module documentation for purpose and usage.
pub struct ModelManifest {
    pub schema_version: u32, pub id: ModelId, pub name: String, pub publisher: String,
    pub license: String, pub license_url: Option<String>, pub acceptable_uses: Vec<UseCase>,
    pub geo_restrictions: Vec<String>, pub file_size_bytes: u64, pub sha256: [u8; 32],
    pub precision: Precision, pub arch: ModelArch, pub total_params: u64, pub active_params: Option<u64>,
    pub dense_core_bytes: u64, pub expert_pool_bytes: u64, pub signature: Option<String>, pub publisher_key_fingerprint: Option<String>,
}
#[async_trait]
/// The `ManifestVerifier` trait — see module documentation for the contract.
pub trait ManifestVerifier: Send + Sync + 'static {
    async fn verify_signature(&self, manifest: &ModelManifest) -> Result<(), EdgeForgeError>;
    async fn verify_integrity(&self, model_path: &Path, expected_sha256: [u8; 32]) -> Result<(), EdgeForgeError> { crate::hash::verify_file_sha256(model_path, expected_sha256).await }
    fn check_license(&self, manifest: &ModelManifest, use_case: UseCase) -> LicenseVerdict;
}
/// `DefaultManifestVerifier` — see module documentation for purpose and usage.
pub struct DefaultManifestVerifier { trusted: Vec<(String, VerifyingKey)> }
impl DefaultManifestVerifier {
    /// Creates a new instance with default configuration.
    pub fn new(trusted: Vec<(String, VerifyingKey)>) -> Self { Self { trusted } }
    /// No trusted keys — see module documentation for details.
    pub fn no_trusted_keys() -> Self { Self { trusted: Vec::new() } }
    fn signed_message(manifest: &ModelManifest) -> Result<Vec<u8>, EdgeForgeError> {
        let mut value = serde_json::to_value(manifest).map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))?;
        if let serde_json::Value::Object(ref mut map) = value { map.remove("signature"); }
        serde_json::to_vec(&value).map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))
    }
}
#[async_trait]
impl ManifestVerifier for DefaultManifestVerifier {
    async fn verify_signature(&self, manifest: &ModelManifest) -> Result<(), EdgeForgeError> {
        let sig_b64 = manifest.signature.as_ref().ok_or_else(|| EdgeForgeError::ManifestSignatureInvalid("no signature".into()))?;
        let sig_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sig_b64).map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))?;
        let sig = Signature::from_slice(&sig_bytes).map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))?;
        let fp = manifest.publisher_key_fingerprint.as_ref().ok_or_else(|| EdgeForgeError::ManifestSignatureInvalid("no fingerprint".into()))?;
        let key = self.trusted.iter().find(|(f, _)| f == fp).map(|(_, k)| *k).ok_or_else(|| EdgeForgeError::ManifestSignatureInvalid("key not trusted".into()))?;
        let msg = Self::signed_message(manifest)?;
        key.verify(&msg, &sig).map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))
    }
    fn check_license(&self, manifest: &ModelManifest, use_case: UseCase) -> LicenseVerdict {
        if manifest.signature.is_none() { return LicenseVerdict::UnsignedRequiresAcknowledgement; }
        if manifest.acceptable_uses.contains(&use_case) { LicenseVerdict::Permitted } else { LicenseVerdict::Forbidden(format!("license '{}' does not permit {:?}", manifest.license, use_case)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgeforge_core::ModelId;

    fn make_manifest() -> ModelManifest {
        ModelManifest {
            schema_version: 1, id: ModelId::new(), name: "Test".into(),
            publisher: "test".into(), license: "Apache-2.0".into(), license_url: None,
            acceptable_uses: vec![], geo_restrictions: vec![], file_size_bytes: 1024,
            sha256: [0u8; 32], precision: edgeforge_core::Precision::Fp16,
            arch: ModelArch::Moe, total_params: 1_000_000_000,
            active_params: Some(50_000_000), dense_core_bytes: 100_000_000,
            expert_pool_bytes: 900_000_000, signature: None, publisher_key_fingerprint: None,
        }
    }

    #[test]
    fn test_manifest_construction() {
        let m = make_manifest();
        assert_eq!(m.name, "Test");
        assert_eq!(m.arch, ModelArch::Moe);
        assert_eq!(m.schema_version, 1);
    }

    #[test]
    fn test_manifest_serializes() {
        let m = make_manifest();
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("moe"));
    }

    #[test]
    fn test_no_trusted_keys_verifier() {
        let v = DefaultManifestVerifier::no_trusted_keys();
        // Should exist without panic
        let _ = format!("{v:?}");
    }

    #[test]
    fn test_license_verdict_unsigned() {
        let v = DefaultManifestVerifier::no_trusted_keys();
        let m = make_manifest();
        let verdict = v.check_license(&m, UseCase::Personal);
        assert_eq!(verdict, LicenseVerdict::UnsignedRequiresAcknowledgement);
    }

    #[test]
    fn test_license_verdict_permitted() {
        let v = DefaultManifestVerifier::no_trusted_keys();
        let mut m = make_manifest();
        m.signature = Some("fake_sig".into());
        m.acceptable_uses = vec![UseCase::Research];
        let verdict = v.check_license(&m, UseCase::Research);
        assert_eq!(verdict, LicenseVerdict::Permitted);
    }

    #[test]
    fn test_license_verdict_forbidden() {
        let v = DefaultManifestVerifier::no_trusted_keys();
        let mut m = make_manifest();
        m.signature = Some("fake_sig".into());
        m.acceptable_uses = vec![UseCase::Research];
        let verdict = v.check_license(&m, UseCase::Commercial);
        assert!(matches!(verdict, LicenseVerdict::Forbidden(_)));
    }

    #[test]
    fn test_use_case_variants() {
        let _ = UseCase::Personal;
        let _ = UseCase::Research;
        let _ = UseCase::Commercial;
    }

    #[test]
    fn test_model_arch_variants() {
        assert_eq!(ModelArch::Moe, ModelArch::Moe);
        assert_ne!(ModelArch::Moe, ModelArch::Dense);
        assert_ne!(ModelArch::Dense, ModelArch::Rwkv);
        assert_ne!(ModelArch::Rwkv, ModelArch::Mamba);
    }
}
