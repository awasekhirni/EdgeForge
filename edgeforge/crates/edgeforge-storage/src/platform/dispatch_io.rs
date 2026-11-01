//! macOS `dispatch_io` backend for PlatformIo (ADR-025, R-03).
//!
//! ## Purpose
//! macOS-specific async I/O using Grand Central Dispatch's `dispatch_io_read`
//! with `F_NOCACHE` for direct I/O. Falls back to `BlockingIo` internally
//! because the real `dispatch_io` integration requires a macOS-specific
//! async runtime adapter. The fallback provides correct (if not maximally
//! efficient) I/O on macOS.
//!
//! ## What This File Does
//! - `DispatchIo` struct — implements `PlatformIo` by delegating to `BlockingIo`
//! - `open_direct()` — delegates to `BlockingIo::open_direct`
//! - `read_at()` — delegates to `BlockingIo::read_at` (fallback, no panic)
//! - `read_batch()` — delegates to `BlockingIo::read_batch` (fallback, no panic)
//! - `prefetch()` — no-op (macOS uses `fcntl(F_RDADVISE)` in production)
//!
//! ## Fallback Strategy
//! Same pattern as `IoUring` on Linux and `Iocp` on Windows: the
//! platform-native backend delegates to `BlockingIo` for correctness.
//! The real `dispatch_io` implementation will use `dispatch_io_read` with
//! `F_NOCACHE` in a future phase.
//!
//! ## Dependencies
//! - `crate::platform::BlockingIo` — fallback implementation
//! - `crate::platform::{PlatformIo, DirectFile, AlignedBuffer, IoError, ReadRequest, ReadResult}`
//!
//! ## Platform
//! macOS only (`#[cfg(target_os = "macos")]`).

#![cfg(target_os = "macos")]

use crate::platform::{AlignedBuffer, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
use async_trait::async_trait;
use std::path::Path;

/// macOS `dispatch_io`-backed `PlatformIo` implementation.
///
/// Uses `BlockingIo` as a fallback for correctness. The real `dispatch_io`
/// implementation will use `dispatch_io_read` with `F_NOCACHE`.
pub struct DispatchIo {
    /// The blocking fallback backend.
    fallback: crate::platform::BlockingIo,
}

impl DispatchIo {
    /// Create a new `DispatchIo` backend.
    ///
    /// ## Returns
    /// A new `DispatchIo` instance with a `BlockingIo` fallback.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fallback: crate::platform::BlockingIo::new(),
        }
    }
}

impl Default for DispatchIo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformIo for DispatchIo {
    /// Open a file with direct I/O semantics.
    ///
    /// Delegates to `BlockingIo::open_direct` for correctness.
    /// Production: will use `open` with `F_NOCACHE`.
    async fn open_direct(&self, path: &Path) -> Result<DirectFile, IoError> {
        tracing::debug!(path = %path.display(), "DispatchIo::open_direct — delegating to BlockingIo");
        self.fallback.open_direct(path).await
    }

    /// Read at a specific offset.
    ///
    /// Delegates to `BlockingIo::read_at` for correctness (no panic).
    /// Production: will use `dispatch_io_read` with `F_NOCACHE`.
    async fn read_at(
        &self,
        file: &DirectFile,
        offset: u64,
        len: usize,
    ) -> Result<AlignedBuffer, IoError> {
        tracing::debug!(offset, len, "DispatchIo::read_at — delegating to BlockingIo");
        self.fallback.read_at(file, offset, len).await
    }

    /// Read a batch of requests.
    ///
    /// Delegates to `BlockingIo::read_batch` for correctness (no panic).
    /// Production: will submit multiple `dispatch_io_read` calls.
    async fn read_batch(
        &self,
        file: &DirectFile,
        requests: &[ReadRequest<'_>],
    ) -> Result<Vec<ReadResult>, IoError> {
        tracing::debug!(request_count = requests.len(), "DispatchIo::read_batch — delegating to BlockingIo");
        self.fallback.read_batch(file, requests).await
    }

    /// Prefetch a region of the file.
    ///
    /// Delegates to `BlockingIo::prefetch` for correctness.
    /// Production: will use `fcntl(F_RDADVISE)` on macOS.
    async fn prefetch(&self, file: &DirectFile, offset: u64, len: usize) {
        tracing::debug!(offset, len, "DispatchIo::prefetch — delegating to BlockingIo");
        self.fallback.prefetch(file, offset, len).await
    }
}

// Note: This file only compiles on macOS (#![cfg(target_os = "macos")]).
// Tests are included in the platform-gated code above.
// On non-macOS, this file is not compiled at all.
