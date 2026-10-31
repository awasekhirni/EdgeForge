// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use crate::platform::{AlignedBuffer, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
use async_trait::async_trait;
use std::path::Path;
use tokio::task;
pub struct BlockingIo;
impl BlockingIo {
    pub fn new() -> Self {
        Self
    }
}
impl Default for BlockingIo {
    fn default() -> Self {
        Self::new()
    }
}
#[async_trait]
impl PlatformIo for BlockingIo {
    async fn open_direct(&self, path: &Path) -> Result<DirectFile, IoError> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|e| IoError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        let size = file
            .metadata()
            .map_err(|e| IoError::Io {
                path: path.to_path_buf(),
                source: e,
            })?
            .len();
        Ok(DirectFile {
            path: path.to_path_buf(),
            size,
        })
    }
    async fn read_at(
        &self,
        file: &DirectFile,
        offset: u64,
        len: usize,
    ) -> Result<AlignedBuffer, IoError> {
        if offset > file.size {
            return Err(IoError::InvalidOffset {
                offset,
                size: file.size,
            });
        }
        let path = file.path().to_path_buf();
        let len = std::cmp::min(len as u64, file.size.saturating_sub(offset)) as usize;
        let buf = task::spawn_blocking(move || -> Result<Vec<u8>, IoError> {
            use std::io::{Read, Seek, SeekFrom};
            let mut f = std::fs::File::open(&path).map_err(|e| IoError::Io {
                path: path.clone(),
                source: e,
            })?;
            f.seek(SeekFrom::Start(offset)).map_err(|e| IoError::Io {
                path: path.clone(),
                source: e,
            })?;
            let mut buf = vec![0u8; len];
            f.read_exact(&mut buf).map_err(|e| IoError::Io {
                path: path.clone(),
                source: e,
            })?;
            Ok(buf)
        })
        .await
        .map_err(|e| IoError::Io {
            path: file.path().to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })??;
        Ok(AlignedBuffer { data: buf })
    }
    async fn read_batch(
        &self,
        file: &DirectFile,
        requests: &[ReadRequest<'_>],
    ) -> Result<Vec<ReadResult>, IoError> {
        let mut out = Vec::with_capacity(requests.len());
        for req in requests {
            let buf = self.read_at(file, req.offset, req.len).await?;
            out.push(ReadResult {
                id: req.id,
                buffer: buf,
            });
        }
        Ok(out)
    }
    async fn prefetch(&self, _file: &DirectFile, _offset: u64, _len: usize) {}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reads_back_written() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        let io = BlockingIo::new();
        let f = io.open_direct(tmp.path()).await.unwrap();
        let buf = io.read_at(&f, 6, 5).await.unwrap();
        assert_eq!(buf.as_slice(), b"world");
    }
}
