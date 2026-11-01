//! # SHA-256 Streaming Integrity Verifier (F-LOAD-04)
//!
//! ## Purpose
//! Stream-hashes a model file in 1 MiB chunks and compares to the expected
//! SHA-256 from the manifest. Memory-bounded — does not buffer the entire file.
//!
//! ## What This File Does
//! - `verify_file_sha256(path, expected)` — async streaming SHA-256 verification
//!
//! ## Dependencies
//! - `ring::digest` — SHA-256 context
//! - `tokio::fs::File` + `tokio::io::BufReader` — async file reading
//! - `hex` — hash encoding for error messages
//!
//! ## Output
//! `Result<(), EdgeForgeError>` — Ok on match, IntegrityMismatch on mismatch.
//!
//! ## Function: verify_file_sha256
//! - **Purpose:** Verify a file's SHA-256 hash matches the expected value
//! - **Parameters:** `path: &Path` — file to hash; `expected: [u8; 32]` — expected SHA-256
//! - **Returns:** `Result<(), EdgeForgeError>` — Ok if match, IntegrityMismatch if not
//! - **Errors:** Io error if file cannot be read; IntegrityMismatch if hash differs
//! - **Dependencies:** `ring::digest::Context` for streaming hash, `tokio::io` for async reads
//! - **Memory:** Uses 1 MiB buffer — bounded regardless of file size

use edgeforge_core::EdgeForgeError;
use ring::digest;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
/// Verify file sha256 — see module documentation for details.
pub async fn verify_file_sha256(path: &Path, expected: [u8; 32]) -> Result<(), EdgeForgeError> {
    let file = File::open(path).await.map_err(|e| EdgeForgeError::Io { path: path.to_path_buf(), source: e })?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut ctx = digest::Context::new(&digest::SHA256);
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = reader.read(&mut buf).await.map_err(|e| EdgeForgeError::Io { path: path.to_path_buf(), source: e })?;
        if n == 0 { break; }
        ctx.update(&buf[..n]);
    }
    let actual = ctx.finish();
    if actual.as_ref() == expected { Ok(()) } else { Err(EdgeForgeError::IntegrityMismatch { expected: hex::encode(expected), actual: hex::encode(actual) }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_verify_matching_hash() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        // SHA-256 of "hello world"
        let expected: [u8; 32] = {
            let mut ctx = digest::Context::new(&digest::SHA256);
            ctx.update(b"hello world");
            let finished = ctx.finish();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(finished.as_ref());
            arr
        };
        let result = verify_file_sha256(tmp.path(), expected).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_mismatched_hash() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        let wrong: [u8; 32] = [0xFF; 32];
        let result = verify_file_sha256(tmp.path(), wrong).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EdgeForgeError::IntegrityMismatch { .. }));
    }

    #[tokio::test]
    async fn test_verify_nonexistent_file() {
        let result = verify_file_sha256(std::path::Path::new("/nonexistent/file"), [0u8; 32]).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EdgeForgeError::Io { .. }));
    }

    #[tokio::test]
    async fn test_verify_empty_file() {
        let tmp = NamedTempFile::new().unwrap();
        // Empty file — SHA-256 of empty = e3b0c44298fc1c149afbf4c8996fb924...
        let expected: [u8; 32] = {
            let mut ctx = digest::Context::new(&digest::SHA256);
            let finished = ctx.finish();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(finished.as_ref());
            arr
        };
        let result = verify_file_sha256(tmp.path(), expected).await;
        assert!(result.is_ok());
    }
}
