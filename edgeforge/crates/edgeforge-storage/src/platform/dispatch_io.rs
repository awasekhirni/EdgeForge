// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! macOS `dispatch_io` backend for PlatformIo (ADR-025, R-03).
//!
//! Phase 5 stub — compiles only on macOS. Uses `dispatch_io_read` with
//! `F_NOCACHE` for direct I/O.

#![cfg(target_os = "macos")]

use crate::platform::{AlignedBuffer, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
use async_trait::async_trait;
use std::path::Path;

/// `DispatchIo` — see module documentation for purpose and usage.
pub struct DispatchIo;

impl DispatchIo {
    #[must_use]
    /// Creates a new instance with default configuration.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DispatchIo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformIo for DispatchIo {
    async fn open_direct(&self, _path: &Path) -> Result<DirectFile, IoError> {
        Err(IoError::Io {
            path: _path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "dispatch_io not yet implemented",
            ),
        })
    }
    async fn read_at(
        &self,
        _file: &DirectFile,
        _offset: u64,
        _len: usize,
    ) -> Result<AlignedBuffer, IoError> {
        unimplemented!("dispatch_io read_at — Phase 5+")
    }
    async fn read_batch(
        &self,
        _file: &DirectFile,
        _requests: &[ReadRequest<'_>],
    ) -> Result<Vec<ReadResult>, IoError> {
        unimplemented!("dispatch_io read_batch — Phase 5+")
    }
    async fn prefetch(&self, _file: &DirectFile, _offset: u64, _len: usize) {
        // macOS: use fcntl(F_RDADVISE)
    }
}
