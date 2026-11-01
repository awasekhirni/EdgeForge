// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! Windows IOCP backend for PlatformIo (ADR-025, R-03).
//!
//! Phase 5 stub — compiles only on Windows. Uses `ReadFile` with
//! `FILE_FLAG_NO_BUFFERING` for direct I/O.

#![cfg(target_os = "windows")]

use crate::platform::{AlignedBuffer, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
use async_trait::async_trait;
use std::path::Path;

/// `Iocp` — see module documentation for purpose and usage.
pub struct Iocp;

impl Iocp {
    #[must_use]
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Iocp {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformIo for Iocp {
    async fn open_direct(&self, _path: &Path) -> Result<DirectFile, IoError> {
        // Phase 5+: implement with CreateFileW + FILE_FLAG_NO_BUFFERING
        Err(IoError::Io {
            path: _path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "IOCP not yet implemented",
            ),
        })
    }
    async fn read_at(
        &self,
        _file: &DirectFile,
        _offset: u64,
        _len: usize,
    ) -> Result<AlignedBuffer, IoError> {
        unimplemented!("IOCP read_at — Phase 5+")
    }
    async fn read_batch(
        &self,
        _file: &DirectFile,
        _requests: &[ReadRequest<'_>],
    ) -> Result<Vec<ReadResult>, IoError> {
        unimplemented!("IOCP read_batch — Phase 5+")
    }
    async fn prefetch(&self, _file: &DirectFile, _offset: u64, _len: usize) {
        // Windows: use PrefetchVirtualMemory or ReclaimFileHandle
    }
}
