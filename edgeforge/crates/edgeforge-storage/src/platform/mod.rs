// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
pub mod blocking;
#[cfg(target_os = "macos")]
pub mod dispatch_io;
#[cfg(target_os = "windows")]
pub mod iocp;
#[cfg(target_os = "linux")]
pub mod iouring;

use async_trait::async_trait;
pub use blocking::BlockingIo;
use edgeforge_core::EdgeForgeError;
#[cfg(target_os = "linux")]
pub use iouring::IoUring;
use std::path::{Path, PathBuf};
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("I/O error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid offset {offset} (size {size})")]
    InvalidOffset { offset: u64, size: u64 },
}
impl From<IoError> for EdgeForgeError {
    fn from(e: IoError) -> Self {
        match e {
            IoError::Io { path, source } => EdgeForgeError::Io { path, source },
            other => EdgeForgeError::Engine(other.to_string()),
        }
    }
}
pub struct DirectFile {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
}
impl DirectFile {
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ReadRequest<'a> {
    pub id: u64,
    pub offset: u64,
    pub len: usize,
    pub _phantom: std::marker::PhantomData<&'a ()>,
}
impl<'a> ReadRequest<'a> {
    pub fn new(id: u64, offset: u64, len: usize) -> Self {
        Self {
            id,
            offset,
            len,
            _phantom: std::marker::PhantomData,
        }
    }
}
#[derive(Debug)]
pub struct ReadResult {
    pub id: u64,
    pub buffer: AlignedBuffer,
}
#[derive(Debug)]
pub struct AlignedBuffer {
    pub data: Vec<u8>,
}
impl AlignedBuffer {
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
#[async_trait]
pub trait PlatformIo: Send + Sync + 'static {
    async fn open_direct(&self, path: &Path) -> Result<DirectFile, IoError>;
    async fn read_at(
        &self,
        file: &DirectFile,
        offset: u64,
        len: usize,
    ) -> Result<AlignedBuffer, IoError>;
    async fn read_batch(
        &self,
        file: &DirectFile,
        requests: &[ReadRequest<'_>],
    ) -> Result<Vec<ReadResult>, IoError>;
    async fn prefetch(&self, file: &DirectFile, offset: u64, len: usize);
}
