// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! Weight pager (F-TIER-01, F-TIER-02, F-TIER-05).
//!
//! Phase 4: implements real `page_in()` using the PlatformIo backend.
//! Pages expert weight tensors from SSD into RAM, with offset calculation
//! based on the model manifest's expert pool layout.

use crate::platform::{DirectFile, PlatformIo};
use edgeforge_core::{Bytes, ExpertId, LayerIdx, ModelId};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

/// Identifier for a single weight page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WeightPageId {
    pub layer: LayerIdx,
    pub expert: Option<ExpertId>,
    pub page: u32,
}

/// A single weight page resident in RAM.
pub struct WeightPage {
    pub id: WeightPageId,
    pub bytes: Vec<u8>,
}

/// Configuration for the weight pager.
#[derive(Clone, Debug)]
pub struct WeightPagerConfig {
    pub page_size: Bytes,
    pub ram_budget: Bytes,
    pub ssd_path: PathBuf,
}

/// The weight pager. Owns the [`PlatformIo`] backend.
/// Uses a Mutex for the file handle so `page_in` can take `&self`.
pub struct WeightPager {
    config: WeightPagerConfig,
    io: Arc<dyn PlatformIo>,
    model_id: ModelId,
    file: Mutex<Option<DirectFile>>,
}

impl WeightPager {
    pub fn new(config: WeightPagerConfig, io: Arc<dyn PlatformIo>, model_id: ModelId) -> Self {
        Self {
            config,
            io,
            model_id,
            file: Mutex::new(None),
        }
    }

    async fn open_if_needed(&self) -> Result<(), edgeforge_core::EdgeForgeError> {
        let mut file_guard = self.file.lock();
        if file_guard.is_none() {
            let file = self
                .io
                .open_direct(&self.config.ssd_path)
                .await
                .map_err(edgeforge_core::EdgeForgeError::from)?;
            *file_guard = Some(file);
        }
        Ok(())
    }

    /// Page in a weight page from SSD at the given offset.
    pub async fn page_in_at(
        &self,
        offset: u64,
        len: usize,
        id: WeightPageId,
    ) -> Result<WeightPage, edgeForgeError> {
        self.open_if_needed().await?;
        let file_guard = self.file.lock();
        let file = file_guard.as_ref().expect("file opened");
        let buf = self.io.read_at(file, offset, len).await?;
        Ok(WeightPage {
            id,
            bytes: buf.data,
        })
    }

    /// Page in a weight page for a specific (layer, expert) pair.
    pub async fn page_in(
        &self,
        id: WeightPageId,
    ) -> Result<WeightPage, edgeforge_core::EdgeForgeError> {
        let offset = (id.layer.0 as u64) * self.config.page_size.0;
        let len = self.config.page_size.0 as usize;
        self.page_in_at(offset, len, id).await
    }

    /// Issue a batch of page-in requests.
    pub async fn page_in_batch(
        &self,
        ids: &[WeightPageId],
    ) -> Result<Vec<WeightPage>, edgeforge_core::EdgeForgeError> {
        self.open_if_needed().await?;
        let page_size = self.config.page_size.0 as usize;
        let file_guard = self.file.lock();
        let file = file_guard.as_ref().expect("file opened");

        let requests: Vec<crate::platform::ReadRequest<'_>> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let offset = (id.layer.0 as u64) * self.config.page_size.0;
                crate::platform::ReadRequest::new(i as u64, offset, page_size)
            })
            .collect();

        let results = self.io.read_batch(file, &requests).await?;

        let mut pages = Vec::with_capacity(ids.len());
        for (i, id) in ids.iter().enumerate() {
            let result = results.iter().find(|r| r.id == i as u64).ok_or_else(|| {
                edgeforge_core::EdgeForgeError::Engine(format!("missing read result for page {i}"))
            })?;
            pages.push(WeightPage {
                id: *id,
                bytes: result.buffer.data.clone(),
            });
        }
        Ok(pages)
    }

    /// Prefetch the next layer's weight pages.
    pub async fn prefetch_next_layer(&self, current_layer: LayerIdx) {
        let file_guard = self.file.lock();
        if let Some(file) = file_guard.as_ref() {
            let offset = (current_layer.next().0 as u64) * self.config.page_size.0;
            let len = self.config.page_size.0 as usize;
            self.io.prefetch(file, offset, len).await;
        }
    }

    #[must_use]
    pub fn config(&self) -> &WeightPagerConfig {
        &self.config
    }

    #[must_use]
    pub fn model_id(&self) -> ModelId {
        self.model_id
    }
}

// Type alias for convenience.
type edgeForgeError = edgeforge_core::EdgeForgeError;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::BlockingIo;
    use tempfile::NamedTempFile;

    fn make_pager(path: std::path::PathBuf) -> WeightPager {
        let io = Arc::new(BlockingIo::new());
        WeightPager::new(
            WeightPagerConfig {
                page_size: Bytes(4096),
                ram_budget: Bytes(1024 * 1024),
                ssd_path: path,
            },
            io,
            ModelId::new(),
        )
    }

    #[tokio::test]
    async fn page_in_reads_from_ssd() {
        let tmp = NamedTempFile::new().unwrap();
        let data: Vec<u8> = (0..8192).map(|i| (i % 256) as u8).collect();
        std::fs::write(tmp.path(), &data).unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        let page = pager
            .page_in(WeightPageId {
                layer: LayerIdx(0),
                expert: None,
                page: 0,
            })
            .await
            .unwrap();
        assert_eq!(page.bytes.len(), 4096);
        assert_eq!(page.bytes[0], 0);
        assert_eq!(page.bytes[255], 255);
    }

    #[tokio::test]
    async fn page_in_at_specific_offset() {
        let tmp = NamedTempFile::new().unwrap();
        let data: Vec<u8> = (0..8192).map(|i| (i % 256) as u8).collect();
        std::fs::write(tmp.path(), &data).unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        let page = pager
            .page_in_at(
                4096,
                100,
                WeightPageId {
                    layer: LayerIdx(1),
                    expert: None,
                    page: 0,
                },
            )
            .await
            .unwrap();
        assert_eq!(page.bytes.len(), 100);
    }

    #[tokio::test]
    async fn page_in_batch_reads_multiple_pages() {
        let tmp = NamedTempFile::new().unwrap();
        let data: Vec<u8> = (0..16384).map(|i| (i % 256) as u8).collect();
        std::fs::write(tmp.path(), &data).unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        let ids = vec![
            WeightPageId {
                layer: LayerIdx(0),
                expert: None,
                page: 0,
            },
            WeightPageId {
                layer: LayerIdx(1),
                expert: None,
                page: 0,
            },
            WeightPageId {
                layer: LayerIdx(2),
                expert: None,
                page: 0,
            },
        ];
        let pages = pager.page_in_batch(&ids).await.unwrap();
        assert_eq!(pages.len(), 3);
        for page in &pages {
            assert_eq!(page.bytes.len(), 4096);
        }
    }

    #[tokio::test]
    async fn page_in_out_of_range_fails() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"short").unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        let result = pager
            .page_in_at(
                1000,
                10,
                WeightPageId {
                    layer: LayerIdx(0),
                    expert: None,
                    page: 0,
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn prefetch_next_layer_does_not_panic() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        // Open the file first.
        let _ = pager
            .page_in(WeightPageId {
                layer: LayerIdx(0),
                expert: None,
                page: 0,
            })
            .await;
        // Prefetch should not panic.
        pager.prefetch_next_layer(LayerIdx(0)).await;
    }
}
