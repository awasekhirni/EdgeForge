// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::EdgeForgeError;
use ring::digest;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
pub async fn verify_file_sha256(path: &Path, expected: [u8; 32]) -> Result<(), EdgeForgeError> {
    let file = File::open(path).await.map_err(|e| EdgeForgeError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut ctx = digest::Context::new(&digest::SHA256);
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|e| EdgeForgeError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    let actual = ctx.finish();
    if actual.as_ref() == expected {
        Ok(())
    } else {
        Err(EdgeForgeError::IntegrityMismatch {
            expected: hex::encode(expected),
            actual: hex::encode(actual),
        })
    }
}
