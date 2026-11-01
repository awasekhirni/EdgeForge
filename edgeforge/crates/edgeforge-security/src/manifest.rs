// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use async_trait::async_trait;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use edgeforge_core::{EdgeForgeError, ModelId, Precision};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseCase {
    Personal,
    Research,
    Commercial,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LicenseVerdict {
    Permitted,
    Forbidden(String),
    UnsignedRequiresAcknowledgement,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelArch {
    Moe,
    Dense,
    Rwkv,
    Mamba,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub id: ModelId,
    pub name: String,
    pub publisher: String,
    pub license: String,
    pub license_url: Option<String>,
    pub acceptable_uses: Vec<UseCase>,
    pub geo_restrictions: Vec<String>,
    pub file_size_bytes: u64,
    pub sha256: [u8; 32],
    pub precision: Precision,
    pub arch: ModelArch,
    pub total_params: u64,
    pub active_params: Option<u64>,
    pub dense_core_bytes: u64,
    pub expert_pool_bytes: u64,
    pub signature: Option<String>,
    pub publisher_key_fingerprint: Option<String>,
}
#[async_trait]
pub trait ManifestVerifier: Send + Sync + 'static {
    async fn verify_signature(&self, manifest: &ModelManifest) -> Result<(), EdgeForgeError>;
    async fn verify_integrity(
        &self,
        model_path: &Path,
        expected_sha256: [u8; 32],
    ) -> Result<(), EdgeForgeError> {
        crate::hash::verify_file_sha256(model_path, expected_sha256).await
    }
    fn check_license(&self, manifest: &ModelManifest, use_case: UseCase) -> LicenseVerdict;
}
pub struct DefaultManifestVerifier {
    trusted: Vec<(String, VerifyingKey)>,
}
impl DefaultManifestVerifier {
    pub fn new(trusted: Vec<(String, VerifyingKey)>) -> Self {
        Self { trusted }
    }
    pub fn no_trusted_keys() -> Self {
        Self {
            trusted: Vec::new(),
        }
    }
    fn signed_message(manifest: &ModelManifest) -> Result<Vec<u8>, EdgeForgeError> {
        let mut value = serde_json::to_value(manifest)
            .map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))?;
        if let serde_json::Value::Object(ref mut map) = value {
            map.remove("signature");
        }
        serde_json::to_vec(&value)
            .map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))
    }
}
#[async_trait]
impl ManifestVerifier for DefaultManifestVerifier {
    async fn verify_signature(&self, manifest: &ModelManifest) -> Result<(), EdgeForgeError> {
        let sig_b64 = manifest
            .signature
            .as_ref()
            .ok_or_else(|| EdgeForgeError::ManifestSignatureInvalid("no signature".into()))?;
        let sig_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sig_b64)
            .map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))?;
        let sig = Signature::from_slice(&sig_bytes)
            .map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))?;
        let fp = manifest
            .publisher_key_fingerprint
            .as_ref()
            .ok_or_else(|| EdgeForgeError::ManifestSignatureInvalid("no fingerprint".into()))?;
        let key = self
            .trusted
            .iter()
            .find(|(f, _)| f == fp)
            .map(|(_, k)| *k)
            .ok_or_else(|| EdgeForgeError::ManifestSignatureInvalid("key not trusted".into()))?;
        let msg = Self::signed_message(manifest)?;
        key.verify(&msg, &sig)
            .map_err(|e| EdgeForgeError::ManifestSignatureInvalid(e.to_string()))
    }
    fn check_license(&self, manifest: &ModelManifest, use_case: UseCase) -> LicenseVerdict {
        if manifest.signature.is_none() {
            return LicenseVerdict::UnsignedRequiresAcknowledgement;
        }
        if manifest.acceptable_uses.contains(&use_case) {
            LicenseVerdict::Permitted
        } else {
            LicenseVerdict::Forbidden(format!(
                "license '{}' does not permit {:?}",
                manifest.license, use_case
            ))
        }
    }
}
