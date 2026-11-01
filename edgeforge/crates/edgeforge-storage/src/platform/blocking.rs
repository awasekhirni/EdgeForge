//! # Portable Blocking I/O Backend
//!
//! ## Purpose
//! Fallback `PlatformIo` implementation using synchronous `pread` under a
//! Tokio blocking pool task. Correct but not optimal — the io_uring/IOCP/
//! dispatch_io backends replace it in production.
//!
//! ## What This File Does
//! - `BlockingIo` struct — implements `PlatformIo`
//! - `open_direct()` — opens file for reading (no O_DIRECT in test mode)
//! - `read_at()` — spawn_blocking + std::fs seek + read
//! - `read_batch()` — sequential implementation of batch reads
//! - `prefetch()` — no-op (production backends use posix_fadvise)
//!
//! ## Dependencies
//! - `tokio::task::spawn_blocking` — offloads sync I/O to blocking pool
//! - `std::fs` — synchronous file operations
//! - `std::io::{Read, Seek}` — seek + read
//!
//! ## Output
//! `AlignedBuffer` with the read data, or `IoError` on failure.
//!
//! ## Function: read_at
//! - **Purpose:** Read `len` bytes at `offset` from the file
//! - **Parameters:** `&DirectFile`, `offset: u64`, `len: usize`
//! - **Returns:** `Result<AlignedBuffer, IoError>`
//! - **Errors:** InvalidOffset if offset > file size; Io on read failure
//! - **Dependencies:** `tokio::task::spawn_blocking`, `std::fs::File`

use crate::platform::{AlignedBuffer, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
use async_trait::async_trait;
use std::path::Path;
use tokio::task;
/// `BlockingIo` — see module documentation for purpose and usage.
pub struct BlockingIo;
impl BlockingIo { pub fn new() -> Self { Self } }
impl Default for BlockingIo { fn default() -> Self { Self::new() } }
#[async_trait]
impl PlatformIo for BlockingIo {
    async fn open_direct(&self, path: &Path) -> Result<DirectFile, IoError> {
        let file = std::fs::OpenOptions::new().read(true).open(path).map_err(|e| IoError::Io { path: path.to_path_buf(), source: e })?;
        let size = file.metadata().map_err(|e| IoError::Io { path: path.to_path_buf(), source: e })?.len();
        Ok(DirectFile { path: path.to_path_buf(), size })
    }
    async fn read_at(&self, file: &DirectFile, offset: u64, len: usize) -> Result<AlignedBuffer, IoError> {
        if offset > file.size { return Err(IoError::InvalidOffset { offset, size: file.size }); }
        let path = file.path().to_path_buf();
        let len = std::cmp::min(len as u64, file.size.saturating_sub(offset)) as usize;
        let buf = task::spawn_blocking(move || -> Result<Vec<u8>, IoError> {
            use std::io::{Read, Seek, SeekFrom};
            let mut f = std::fs::File::open(&path).map_err(|e| IoError::Io { path: path.clone(), source: e })?;
            f.seek(SeekFrom::Start(offset)).map_err(|e| IoError::Io { path: path.clone(), source: e })?;
            let mut buf = vec![0u8; len];
            f.read_exact(&mut buf).map_err(|e| IoError::Io { path: path.clone(), source: e })?;
            Ok(buf)
        }).await.map_err(|e| IoError::Io { path: file.path().to_path_buf(), source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()) })??;
        Ok(AlignedBuffer { data: buf })
    }
    async fn read_batch(&self, file: &DirectFile, requests: &[ReadRequest<'_>]) -> Result<Vec<ReadResult>, IoError> {
        let mut out = Vec::with_capacity(requests.len());
        for req in requests { let buf = self.read_at(file, req.offset, req.len).await?; out.push(ReadResult { id: req.id, buffer: buf }); }
        Ok(out)
    }
    async fn prefetch(&self, _file: &DirectFile, _offset: u64, _len: usize) {}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test] async fn reads_back_written() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        let io = BlockingIo::new();
        let f = io.open_direct(tmp.path()).await.unwrap();
        let buf = io.read_at(&f, 6, 5).await.unwrap();
        assert_eq!(buf.as_slice(), b"world");
    }
}
