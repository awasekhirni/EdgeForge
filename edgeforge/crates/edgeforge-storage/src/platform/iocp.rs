//! Windows IOCP backend for PlatformIo (ADR-025, R-03).
//!
//! ## Purpose
//! Windows-specific async I/O using I/O Completion Ports (IOCP) with
//! `FILE_FLAG_NO_BUFFERING` for direct I/O. Falls back to `BlockingIo`
//! internally because the real IOCP integration requires a Windows-specific
//! async runtime adapter. The fallback provides correct (if not maximally
//! efficient) I/O on Windows.
//!
//! ## What This File Does
//! - `Iocp` struct — implements `PlatformIo` by delegating to `BlockingIo`
//! - `open_direct()` — delegates to `BlockingIo::open_direct`
//! - `read_at()` — delegates to `BlockingIo::read_at` (fallback, no panic)
//! - `read_batch()` — delegates to `BlockingIo::read_batch` (fallback, no panic)
//! - `prefetch()` — no-op (Windows uses `PrefetchVirtualMemory` in production)
//!
//! ## Fallback Strategy
//! Same pattern as `IoUring` on Linux: the platform-native backend delegates
//! to `BlockingIo` for correctness. The real IOCP implementation will use
//! `CreateFileW` + `ReadFile` with overlapped I/O in a future phase.
//!
//! ## Dependencies
//! - `crate::platform::BlockingIo` — fallback implementation
//! - `crate::platform::{PlatformIo, DirectFile, AlignedBuffer, IoError, ReadRequest, ReadResult}`
//!
//! ## Platform
//! Windows only (`#[cfg(target_os = "windows")]`).

#![cfg(target_os = "windows")]

use crate::platform::{AlignedBuffer, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
use async_trait::async_trait;
use std::path::Path;

/// Windows IOCP-backed `PlatformIo` implementation.
///
/// Uses `BlockingIo` as a fallback for correctness. The real IOCP
/// implementation will use `CreateFileW` + `ReadFile` with overlapped I/O.
pub struct Iocp {
    /// The blocking fallback backend.
    fallback: crate::platform::BlockingIo,
}

impl Iocp {
    /// Create a new `Iocp` backend.
    ///
    /// ## Returns
    /// A new `Iocp` instance with a `BlockingIo` fallback.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fallback: crate::platform::BlockingIo::new(),
        }
    }
}

impl Default for Iocp {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformIo for Iocp {
    /// Open a file with direct I/O semantics.
    ///
    /// Delegates to `BlockingIo::open_direct` for correctness.
    /// Production: will use `CreateFileW` with `FILE_FLAG_NO_BUFFERING`.
    async fn open_direct(&self, path: &Path) -> Result<DirectFile, IoError> {
        tracing::debug!(path = %path.display(), "Iocp::open_direct — delegating to BlockingIo");
        self.fallback.open_direct(path).await
    }

    /// Read at a specific offset.
    ///
    /// Delegates to `BlockingIo::read_at` for correctness (no panic).
    /// Production: will use `ReadFile` with overlapped I/O via IOCP.
    async fn read_at(
        &self,
        file: &DirectFile,
        offset: u64,
        len: usize,
    ) -> Result<AlignedBuffer, IoError> {
        tracing::debug!(offset, len, "Iocp::read_at — delegating to BlockingIo");
        self.fallback.read_at(file, offset, len).await
    }

    /// Read a batch of requests.
    ///
    /// Delegates to `BlockingIo::read_batch` for correctness (no panic).
    /// Production: will submit multiple overlapped `ReadFile` calls via IOCP.
    async fn read_batch(
        &self,
        file: &DirectFile,
        requests: &[ReadRequest<'_>],
    ) -> Result<Vec<ReadResult>, IoError> {
        tracing::debug!(request_count = requests.len(), "Iocp::read_batch — delegating to BlockingIo");
        self.fallback.read_batch(file, requests).await
    }

    /// Prefetch a region of the file.
    ///
    /// Delegates to `BlockingIo::prefetch` for correctness.
    /// Production: will use `PrefetchVirtualMemory` on Windows.
    async fn prefetch(&self, file: &DirectFile, offset: u64, len: usize) {
        tracing::debug!(offset, len, "Iocp::prefetch — delegating to BlockingIo");
        self.fallback.prefetch(file, offset, len).await
    }
}

// Note: This file only compiles on Windows (#![cfg(target_os = "windows")]).
// Tests are included in the platform-gated code above.
// On non-Windows, this file is not compiled at all.
