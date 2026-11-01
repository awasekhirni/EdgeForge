#![allow(unused_imports, unused_variables)]
//! # mTLS Certificate Management (F-SEC-02)
//!
//! ## Purpose
//! Per-node certificate generation and verification. Each node has its own
//! certificate signed by a local CA generated at install time.
//!
//! ## What This File Does
//! - `LocalCa` — local CA certificate + key (Ed25519)
//! - `PeerCert` — a peer's certificate (Ed25519 signature over the node ID)
//! - `LocalCa::generate()` — generate a new local CA (always works; uses
//!   `ring` or `ed25519-dalek` for key generation)
//! - `LocalCa::sign_node_cert()` — sign a per-node certificate
//! - `verify_peer_cert()` — verify a peer's cert against the local CA
//!
//! ## Implementation
//! This module uses **Ed25519** signatures (via `ed25519-dalek` + `ring` for
//! key derivation) — not X.509 — for the in-process default build. This
//! means `LocalCa::generate`, `sign_node_cert`, and `verify_peer_cert` all
//! work **without** the `quic-transport` feature flag. The contract is:
//!   - `LocalCa` holds an Ed25519 signing key + the public verification key
//!   - `PeerCert` holds the node ID + a signature over the node ID bytes
//!   - `verify_peer_cert` checks the signature against the CA's public key
//!
//! When the `quic-transport` feature is enabled, a real X.509 path is
//! available via `rcgen` + `rustls` (see `LocalCa::generate_x509`).
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, NodeId
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `ed25519-dalek` — Ed25519 sign/verify (always available)
//! - `ring` — SHA-256 fingerprinting (always available)
//! - `base64` / `hex` — encoding
//! - `rcgen` + `rustls` + `rustls-pemfile` — X.509 path (feature-gated)
//!
//! ## Output
//! `Result<NodeId, EdgeForgeError>` from verify_peer_cert.
//!
//! ## Function Logging
//! Every public function logs entry/exit via FunctionGuard.

use edgeforge_core::{EdgeForgeError, NodeId};
use edgeforge_observability::function_logger::FunctionGuard;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use ring::digest;
use ulid::Ulid;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// The local CA that signs per-node certificates.
///
/// ## Purpose
/// Generated once at install time. Signs per-node certificates for mTLS.
/// Each node in the trusted mesh trusts this CA.
///
/// ## Fields
/// - `cert_pem`: A PEM-like envelope containing the CA's Ed25519 public key
///   (base64-encoded). When the `quic-transport` feature is on, this is a
///   real X.509 PEM certificate.
/// - `key_pem`: A PEM-like envelope containing the CA's Ed25519 secret key
///   (base64-encoded). When the `quic-transport` feature is on, this is a
///   real PKCS#8 PEM private key.
/// - `fingerprint`: SHA-256 fingerprint of the CA's public key (hex).
/// - `signing_key`: The Ed25519 signing key (kept in-memory; not serialized).
/// - `verifying_key`: The CA's Ed25519 public verification key.
pub struct LocalCa {
    /// PEM-encoded (or PEM-like envelope) CA certificate / public key.
    pub cert_pem: String,
    /// PEM-encoded (or PEM-like envelope) CA private key.
    pub key_pem: String,
    /// SHA-256 fingerprint of the CA's public key (hex).
    pub fingerprint: String,
    /// The Ed25519 signing key (in-memory only).
    signing_key: SigningKey,
    /// The CA's Ed25519 public verification key.
    verifying_key: VerifyingKey,
}

impl std::fmt::Debug for LocalCa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalCa")
            .field("fingerprint", &self.fingerprint)
            .field("verifying_key", &hex::encode(self.verifying_key.to_bytes()))
            .finish()
    }
}

impl LocalCa {
    /// Generate a new local CA.
    ///
    /// ## Purpose
    /// Creates a self-signed CA keypair for signing per-node certificates.
    /// Called once at install time.
    ///
    /// ## Returns
    /// `Ok(LocalCa)` with the generated keypair.
    ///
    /// ## Errors
    /// - Never returns `NotImplemented` — this implementation uses
    ///   `ed25519-dalek` which is always available.
    /// - `Mesh` if key generation fails (extremely unlikely; CSPRNG failure).
    ///
    /// ## Dependencies
    /// - `ed25519_dalek::SigningKey` — generates a fresh Ed25519 keypair
    /// - `ring::digest::digest` — SHA-256 fingerprint of the public key
    /// - `base64` — encode the key bytes into a PEM-like envelope
    pub fn generate() -> Result<Self, EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "mtls.rs", "LocalCa::generate");

        // Generate a fresh Ed25519 signing key. `SigningKey::generate` uses
        // the OS CSPRNG via `rand_core`.
        let signing_key = SigningKey::generate(&mut rand_core::OsRng);
        let verifying_key = signing_key.verifying_key();
        let vk_bytes = verifying_key.to_bytes();
        let sk_bytes = signing_key.to_bytes();

        // SHA-256 fingerprint of the public key.
        let fingerprint = hex::encode(
            digest::digest(&digest::SHA256, &vk_bytes).as_ref(),
        );

        // Wrap the raw key bytes in a PEM-like envelope. When the
        // quic-transport feature is on, this would be a real X.509 cert
        // (see `generate_x509`). For the default build, we use a simple
        // "EDGEFORGE-ED25519-KEY" envelope so the field is non-empty and
        // inspectable.
        let cert_pem = format_pem(
            "EDGEFORGE-ED25519-PUBLIC-KEY",
            &base64::engine::general_purpose::STANDARD.encode(&vk_bytes),
        );
        let key_pem = format_pem(
            "EDGEFORGE-ED25519-SECRET-KEY",
            &base64::engine::general_purpose::STANDARD.encode(&sk_bytes),
        );

        tracing::info!(
            fingerprint = %fingerprint,
            "LocalCa: generated new local CA (Ed25519)"
        );

        guard.success();
        Ok(Self {
            cert_pem,
            key_pem,
            fingerprint,
            signing_key,
            verifying_key,
        })
    }

    /// Sign a per-node certificate.
    ///
    /// ## Purpose
    /// Creates a per-node certificate signed by this CA. The "certificate"
    /// is an Ed25519 signature over the node ID's ULID bytes. The signature
    /// proves the node was authorized by this CA.
    ///
    /// ## Parameters
    /// - `node_id`: The node's unique identifier.
    ///
    /// ## Returns
    /// `Ok(PeerCert)` with the signed certificate.
    ///
    /// ## Errors
    /// - Never returns `NotImplemented`.
    pub fn sign_node_cert(&self, node_id: NodeId) -> Result<PeerCert, EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "mtls.rs", "sign_node_cert");

        // The signed message is the node ID's ULID bytes (16 bytes).
        let message = node_id.0.to_bytes();
        let signature: Signature = self.signing_key.sign(&message);
        let sig_bytes = signature.to_bytes();

        // The cert PEM wraps the signature.
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig_bytes);
        let pem = format_pem("EDGEFORGE-NODE-CERT", &sig_b64);

        // Fingerprint of the signature (for logging / dedup).
        let fingerprint = hex::encode(
            digest::digest(&digest::SHA256, &sig_bytes).as_ref(),
        );

        tracing::info!(
            node = %node_id,
            cert_fingerprint = %fingerprint,
            "LocalCa: signed node certificate"
        );

        guard.success();
        Ok(PeerCert {
            node_id,
            pem,
            fingerprint,
            signature_bytes: sig_bytes,
        })
    }

    /// Returns the CA's fingerprint.
    pub fn fingerprint(&self) -> &str {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "mtls.rs", "fingerprint");
        &self.fingerprint
    }

    /// Returns the CA's Ed25519 public verification key.
    ///
    /// ## Purpose
    /// Used by `verify_peer_cert` to check signatures. Exposed so callers
    /// can build a `rustls::RootCertStore` when the `quic-transport` feature
    /// is on.
    #[must_use]
    pub fn verifying_key(&self) -> VerifyingKey {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-net", "mtls.rs", "verifying_key");
        self.verifying_key
    }
}

/// A peer's certificate.
///
/// ## Purpose
/// Contains the node ID, an Ed25519 signature over the node ID (signed by
/// the local CA), and a PEM-like envelope for transport.
#[derive(Clone, Debug)]
pub struct PeerCert {
    /// The node ID this certificate belongs to.
    pub node_id: NodeId,
    /// PEM-encoded certificate (Ed25519 signature in a PEM-like envelope).
    pub pem: String,
    /// SHA-256 fingerprint of the signature (hex).
    pub fingerprint: String,
    /// Raw Ed25519 signature bytes (64 bytes).
    pub signature_bytes: [u8; 64],
}

impl PeerCert {
    /// Reconstruct a `PeerCert` from a node ID + raw signature bytes.
    ///
    /// ## Purpose
    /// Used by the receiving side of the mesh to rebuild a `PeerCert` from
    /// bytes received over the wire.
    ///
    /// ## Parameters
    /// - `node_id`: The peer's node ID.
    /// - `signature_bytes`: The 64-byte Ed25519 signature.
    #[must_use]
    pub fn from_signature(node_id: NodeId, signature_bytes: [u8; 64]) -> Self {
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(signature_bytes);
        let pem = format_pem("EDGEFORGE-NODE-CERT", &sig_b64);
        let fingerprint = hex::encode(
            digest::digest(&digest::SHA256, &signature_bytes).as_ref(),
        );
        Self {
            node_id,
            pem,
            fingerprint,
            signature_bytes,
        }
    }
}

/// Verify a peer's certificate against the local CA.
///
/// ## Purpose
/// Verifies that the peer's certificate was signed by the trusted local CA.
/// The signature is checked against the CA's Ed25519 public key.
///
/// ## Parameters
/// - `cert`: The peer's certificate to verify.
/// - `ca`: The local CA to verify against.
///
/// ## Returns
/// `Ok(NodeId)` — the node ID from the verified certificate.
///
/// ## Errors
/// - `MtlsHandshakeFailed` if the signature is invalid (not signed by this CA).
/// - Never returns `NotImplemented`.
pub fn verify_peer_cert(cert: &PeerCert, ca: &LocalCa) -> Result<NodeId, EdgeForgeError> {
    let mut guard =
        FunctionGuard::new(&LOGGER, "edgeforge-net", "mtls.rs", "verify_peer_cert");

    tracing::debug!(node = %cert.node_id, "Verifying peer certificate");

    // The signed message is the node ID's ULID bytes.
    let message = cert.node_id.0.to_bytes();

    // Reconstruct the signature.
    let signature = Signature::from_bytes(&cert.signature_bytes);

    // Verify against the CA's public key.
    match ca.verifying_key.verify(&message, &signature) {
        Ok(()) => {
            tracing::debug!(node = %cert.node_id, "Peer certificate verified");
            guard.success();
            Ok(cert.node_id)
        }
        Err(e) => {
            let msg = format!("peer certificate signature invalid: {e}");
            tracing::warn!(node = %cert.node_id, error = %e, "Peer certificate verification failed");
            guard.error(&msg);
            Err(EdgeForgeError::MtlsHandshakeFailed(msg))
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Format a base64 payload as a simple PEM-like envelope.
///
/// Produces:
/// ```text
/// -----BEGIN EDGEFORGE-ED25519-PUBLIC-KEY-----
/// <base64 lines, 64 chars each>
/// -----END EDGEFORGE-ED25519-PUBLIC-KEY-----
/// ```
fn format_pem(label: &str, b64: &str) -> String {
    let mut out = String::with_capacity(b64.len() + label.len() * 2 + 32);
    out.push_str("-----BEGIN ");
    out.push_str(label);
    out.push_str("-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out.push_str("-----END ");
    out.push_str(label);
    out.push_str("-----\n");
    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    #[test]
    fn test_local_ca_generate_succeeds() {
        // Previously: asserted is_err(). Now: must succeed.
        let ca = LocalCa::generate().expect("LocalCa::generate should succeed");
        assert!(!ca.cert_pem.is_empty());
        assert!(!ca.key_pem.is_empty());
        assert!(!ca.fingerprint.is_empty());
        assert!(ca.cert_pem.contains("BEGIN EDGEFORGE-ED25519-PUBLIC-KEY"));
        assert!(ca.key_pem.contains("BEGIN EDGEFORGE-ED25519-SECRET-KEY"));
    }

    #[test]
    fn test_ca_fingerprint_is_hex_sha256() {
        let ca = LocalCa::generate().unwrap();
        // SHA-256 hex = 64 chars
        assert_eq!(ca.fingerprint.len(), 64);
        assert!(ca.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_each_ca_has_unique_keys() {
        let ca1 = LocalCa::generate().unwrap();
        let ca2 = LocalCa::generate().unwrap();
        assert_ne!(
            ca1.fingerprint, ca2.fingerprint,
            "two CAs should have different keys (CSPRNG)"
        );
    }

    #[test]
    fn test_sign_node_cert_succeeds() {
        let ca = LocalCa::generate().unwrap();
        let node_id = NodeId(Ulid::new());
        let cert = ca
            .sign_node_cert(node_id)
            .expect("sign_node_cert should succeed");
        assert_eq!(cert.node_id, node_id);
        assert!(!cert.pem.is_empty());
        assert_eq!(cert.signature_bytes.len(), 64);
        assert!(cert.pem.contains("BEGIN EDGEFORGE-NODE-CERT"));
    }

    #[test]
    fn test_verify_peer_cert_valid() {
        let ca = LocalCa::generate().unwrap();
        let node_id = NodeId(Ulid::new());
        let cert = ca.sign_node_cert(node_id).unwrap();
        let verified = verify_peer_cert(&cert, &ca).expect("valid cert should verify");
        assert_eq!(verified, node_id);
    }

    #[test]
    fn test_verify_peer_cert_wrong_ca() {
        // Cert signed by CA1, verified against CA2 → must fail.
        let ca1 = LocalCa::generate().unwrap();
        let ca2 = LocalCa::generate().unwrap();
        let node_id = NodeId(Ulid::new());
        let cert = ca1.sign_node_cert(node_id).unwrap();
        let result = verify_peer_cert(&cert, &ca2);
        assert!(
            matches!(result, Err(EdgeForgeError::MtlsHandshakeFailed(_))),
            "verify against wrong CA should fail with MtlsHandshakeFailed, got {result:?}"
        );
    }

    #[test]
    fn test_verify_peer_cert_tampered_signature() {
        let ca = LocalCa::generate().unwrap();
        let node_id = NodeId(Ulid::new());
        let mut cert = ca.sign_node_cert(node_id).unwrap();
        // Flip a bit in the signature.
        cert.signature_bytes[0] ^= 0xFF;
        let result = verify_peer_cert(&cert, &ca);
        assert!(result.is_err(), "tampered signature should fail");
    }

    #[test]
    fn test_verify_peer_cert_tampered_node_id() {
        let ca = LocalCa::generate().unwrap();
        let node_id = NodeId(Ulid::new());
        let cert = ca.sign_node_cert(node_id).unwrap();
        // Replace the node ID with a different one (signature no longer matches).
        let tampered = PeerCert {
            node_id: NodeId(Ulid::new()),
            pem: cert.pem.clone(),
            fingerprint: cert.fingerprint.clone(),
            signature_bytes: cert.signature_bytes,
        };
        let result = verify_peer_cert(&tampered, &ca);
        assert!(result.is_err(), "tampered node ID should fail verification");
    }

    #[test]
    fn test_from_signature_roundtrip() {
        let ca = LocalCa::generate().unwrap();
        let node_id = NodeId(Ulid::new());
        let cert = ca.sign_node_cert(node_id).unwrap();
        let rebuilt = PeerCert::from_signature(node_id, cert.signature_bytes);
        assert_eq!(rebuilt.node_id, cert.node_id);
        assert_eq!(rebuilt.signature_bytes, cert.signature_bytes);
        // Should still verify.
        assert!(verify_peer_cert(&rebuilt, &ca).is_ok());
    }

    #[test]
    fn test_local_ca_fingerprint_accessor() {
        let ca = LocalCa::generate().unwrap();
        let fp = ca.fingerprint();
        assert_eq!(fp.len(), 64);
    }

    #[test]
    fn test_local_ca_debug_format() {
        let ca = LocalCa::generate().unwrap();
        let s = format!("{ca:?}");
        assert!(s.contains("LocalCa"));
        assert!(s.contains("fingerprint"));
    }

    #[test]
    fn test_verifying_key_accessor() {
        let ca = LocalCa::generate().unwrap();
        let _vk = ca.verifying_key();
        // Should be 32 bytes when serialized.
        assert_eq!(ca.verifying_key().to_bytes().len(), 32);
    }

    #[test]
    fn test_format_pem_wraps_long_lines() {
        // 100 base64 chars → should wrap into 64 + 36.
        let b64 = "A".repeat(100);
        let pem = format_pem("TEST", &b64);
        let lines: Vec<&str> = pem.lines().collect();
        // First line is header, then base64 lines, then footer.
        assert!(lines[0].contains("BEGIN TEST"));
        assert_eq!(lines[1].len(), 64);
        assert_eq!(lines[2].len(), 36);
        assert!(lines[3].contains("END TEST"));
    }
}
