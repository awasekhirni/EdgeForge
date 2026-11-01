#![allow(unused_imports, unused_variables, dead_code)]
//! # edgeforge-storage — L3 Storage Tier
//!
//! ## Purpose
//! The heart of EdgeForge. Implements weight paging, KV cache paging (with SSD
//! spill), multi-horizon prefetching, and platform-native async I/O.
//!
//! ## Modules
//! - `platform/` — PlatformIo trait + BlockingIo (portable) + IoUring (Linux)
//! - `weight_pager` — F-TIER-01/02/05 expert weight paging from SSD
//! - `expert_cache` — RAM-resident LRU expert cache
//! - `kv_pager` — F-TIER-04/06 paged KV cache with SSD spill
//! - `prefetcher` — F-TIER-03 layer-ahead prefetcher
//! - `thermal` — F-MON-04 SSD temperature monitoring
//! - `vram_tier` — VRAM staging (CUDA/Metal/ROCm)
//! - `wear` — F-MON-03 SSD health metrics
//!
//! ## Dependencies
//! - `edgeforge-core` — Bytes, ExpertId, LayerIdx, EdgeForgeError
//! - `async-trait`, `tokio` — async I/O
//! - `parking_lot` — thread-safe mutexes
//! - `libc` — posix_fadvise (Linux prefetch)
//! - `io-uring` (Linux only) — async I/O backend
//!
//! ## Output
//! Weight pages, KV cache pages, and thermal/health metrics for the inference engine.

pub mod expert_cache;
pub mod kv_pager;
pub mod platform;
pub mod prefetcher;
pub mod thermal;
pub mod vram_tier;
pub mod wear;
pub mod weight_pager;
pub use expert_cache::{ExpertCache, ExpertCacheConfig};
pub use platform::{AlignedBuffer, BlockingIo, DirectFile, IoError, PlatformIo, ReadRequest, ReadResult};
#[cfg(target_os = "linux")]
pub use platform::IoUring;
pub use thermal::{ThermalMonitor, ThermalPolicy, ThermalState};
