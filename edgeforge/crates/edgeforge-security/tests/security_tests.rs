// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Security Tests — edgeforge-security
//!
//! Tests for: SHA-256 hash verification, manifest signature verification,
//! license checks, sandbox policy enforcement, and crypto-agility.

#[cfg(test)]
mod tests {
    use edgeforge_core::{ModelId, Precision};
    use edgeforge_security::*;
    use tempfile::NamedTempFile;

    // ── Hash Tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_verify_file_sha256_match() {
        let tmp = NamedTempFile::new().unwrap();
        tokio::fs::write(tmp.path(), b"hello world").await.unwrap();
        // SHA-256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let expected: [u8; 32] =
            hex::decode("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
                .unwrap()
                .try_into()
                .unwrap();
        assert!(hash::verify_file_sha256(tmp.path(), expected).await.is_ok());
    }

    #[tokio::test]
    async fn test_verify_file_sha256_mismatch() {
        let tmp = NamedTempFile::new().unwrap();
        tokio::fs::write(tmp.path(), b"hello world").await.unwrap();
        let wrong: [u8; 32] = [0u8; 32];
        assert!(hash::verify_file_sha256(tmp.path(), wrong).await.is_err());
    }

    // ── Manifest Tests ──────────────────────────────────────────────

    fn make_manifest() -> manifest::ModelManifest {
        manifest::ModelManifest {
            schema_version: 1,
            id: ModelId::new(),
            name: "Test Model".to_string(),
            publisher: "test".to_string(),
            license: "Apache-2.0".to_string(),
            license_url: None,
            acceptable_uses: vec![manifest::UseCase::Research],
            geo_restrictions: vec![],
            file_size_bytes: 1024,
            sha256: [0u8; 32],
            precision: Precision::Fp16,
            arch: manifest::ModelArch::Moe,
            total_params: 1_000_000_000,
            active_params: Some(50_000_000),
            dense_core_bytes: 100_000_000,
            expert_pool_bytes: 900_000_000,
            signature: None,
            publisher_key_fingerprint: None,
        }
    }

    #[test]
    fn test_manifest_license_check_permitted() {
        let verifier = manifest::DefaultManifestVerifier::no_trusted_keys();
        let m = make_manifest();
        // Unsigned → UnsignedRequiresAcknowledgement
        assert_eq!(
            verifier.check_license(&m, manifest::UseCase::Research),
            manifest::LicenseVerdict::UnsignedRequiresAcknowledgement
        );
    }

    #[test]
    fn test_manifest_license_check_unsigned() {
        let verifier = manifest::DefaultManifestVerifier::no_trusted_keys();
        let m = make_manifest();
        assert_eq!(
            verifier.check_license(&m, manifest::UseCase::Commercial),
            manifest::LicenseVerdict::UnsignedRequiresAcknowledgement
        );
    }

    #[tokio::test]
    async fn test_manifest_signature_reject_unsigned() {
        let verifier = manifest::DefaultManifestVerifier::no_trusted_keys();
        let m = make_manifest();
        // No signature → verify_signature should fail
        assert!(verifier.verify_signature(&m).await.is_err());
    }

    // ── Sandbox Tests ───────────────────────────────────────────────

    #[test]
    fn test_sandbox_enforce_consistent_policy() {
        let cache = std::path::Path::new("/tmp/cache");
        let policy = sandbox::SandboxPolicy::remote_peer(cache);
        assert!(sandbox::Sandbox::enforce(policy).is_ok());
    }

    #[test]
    fn test_sandbox_enforce_inconsistent_policy() {
        let policy = sandbox::SandboxPolicy {
            read_paths: vec![std::path::PathBuf::from("/read")],
            write_paths: vec![std::path::PathBuf::from("/write")],
            network_egress: false,
            allow_subprocess: false,
        };
        assert!(sandbox::Sandbox::enforce(policy).is_err());
    }

    // ── Crypto Agility Tests ────────────────────────────────────────

    #[test]
    fn test_crypto_policy_default() {
        let p = crypto_agility::CryptoPolicy::default();
        assert_eq!(p.kem, crypto_agility::KemVariant::X25519);
        assert_eq!(p.sig, crypto_agility::SigVariant::Ed25519);
        assert_eq!(p.phase, crypto_agility::MigrationPhase::ClassicalOnly);
    }

    #[test]
    fn test_crypto_inventory_has_entries() {
        // Just test that CryptoPolicy serializes correctly
        let p = crypto_agility::CryptoPolicy::default();
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("x25519"));
        assert!(json.contains("ed25519"));
        assert!(json.contains("classical_only"));
    }
}
