#![allow(unused_imports, unused_variables)]
//! # Weight Pager (F-TIER-01, F-TIER-02, F-TIER-05)
//!
//! ## Purpose
//! Pages expert weight tensors from SSD into RAM. The dense core is pinned in
//! RAM at load time; experts are paged on demand based on the MoE router's
//! top-k selection.
//!
//! ## What This File Does
//! - `WeightPageId` — identifies a page (layer + expert + page index)
//! - `WeightPage` — a resident page (id + bytes)
//! - `WeightPagerConfig` — page_size, ram_budget, ssd_path
//! - `WeightPager` — owns PlatformIo backend, opens file lazily
//! - `page_in(id)` — reads a page at layer-based offset
//! - `page_in_at(offset, len, id)` — reads at a specific offset
//! - `page_in_batch(ids)` — batch read using PlatformIo::read_batch
//! - `prefetch_next_layer(layer)` — OS prefetch hint for layer N+1
//!
//! ## Dependencies
//! - `crate::platform::PlatformIo` — async I/O backend
//! - `parking_lot::Mutex` — thread-safe lazy file opening
//! - `edgeforge_core::{Bytes, ExpertId, LayerIdx, ModelId}`
//!
//! ## Output
//! `WeightPage` containing the read bytes, or `EdgeForgeError` on failure.
//!
//! ## Function: page_in
//! - **Purpose:** Read a weight page for a specific (layer, expert) pair
//! - **Parameters:** `WeightPageId` — identifies which page to read
//! - **Returns:** `Result<WeightPage, EdgeForgeError>`
//! - **Errors:** Io if file cannot be read; InvalidOffset if offset out of range
//! - **Dependencies:** `self.ensure_open()`, `self.io.read_at()`
//! - **Offset calculation:** `layer * page_size` (simplified; real impl uses manifest layout)
//!
//! ## Function: page_in_batch
//! - **Purpose:** Issue a batch of page-in requests for higher throughput
//! - **Parameters:** `&[WeightPageId]` — pages to read
//! - **Returns:** `Result<Vec<WeightPage>, EdgeForgeError>`
//! - **Dependencies:** `self.io.read_batch()` — single io_uring SQE chain
//! - **Correlation:** Results are correlated by index (0-based) to input IDs

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
    /// Creates a new instance with default configuration.
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
    ) -> Result<WeightPage, EdgeForgeErrorAlias> {
        self.open_if_needed().await?;
        let file_guard = self.file.lock();
        let file = file_guard.as_ref().expect("file opened");
        let buf = self.io.read_at(file, offset, len).await?;
        Ok(WeightPage { id, bytes: buf.data })
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
            let result = results
                .iter()
                .find(|r| r.id == i as u64)
                .ok_or_else(|| edgeforge_core::EdgeForgeError::Engine(format!("missing read result for page {i}")))?;
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
    /// Config — see module documentation for details.
    pub fn config(&self) -> &WeightPagerConfig {
        &self.config
    }

    #[must_use]
    /// Model id — see module documentation for details.
    pub fn model_id(&self) -> ModelId {
        self.model_id
    }
}

// Type alias for convenience.
type EdgeForgeErrorAlias = edgeforge_core::EdgeForgeError;

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
            .page_in(WeightPageId { layer: LayerIdx(0), expert: None, page: 0 })
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
            .page_in_at(4096, 100, WeightPageId { layer: LayerIdx(1), expert: None, page: 0 })
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
            WeightPageId { layer: LayerIdx(0), expert: None, page: 0 },
            WeightPageId { layer: LayerIdx(1), expert: None, page: 0 },
            WeightPageId { layer: LayerIdx(2), expert: None, page: 0 },
        ];
        let pages = pager.page_in_batch(&ids).await.unwrap();
        assert_eq!(pages.len(), 3);
        for page in &pages { assert_eq!(page.bytes.len(), 4096); }
    }

    #[tokio::test]
    async fn page_in_out_of_range_fails() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"short").unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        let result = pager
            .page_in_at(1000, 10, WeightPageId { layer: LayerIdx(0), expert: None, page: 0 })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn prefetch_next_layer_does_not_panic() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        let pager = make_pager(tmp.path().to_path_buf());
        // Open the file first.
        let _ = pager.page_in(WeightPageId { layer: LayerIdx(0), expert: None, page: 0 }).await;
        // Prefetch should not panic.
        pager.prefetch_next_layer(LayerIdx(0)).await;
    }
}
