// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Preset Resolver (F-UX-04)
//!
//! ## Purpose
//! Maps user-facing presets (Quality/Balanced/Fast/MaxQuality) to concrete
//! engine/storage parameters (precision, prefetch depth, KV cache budget).
//!
//! ## What This File Does
//! - `ResolvedPreset` — name, precision, prefetch_depth_layers, kv_cache_budget_gib
//! - `resolve(preset)` — resolve a preset to concrete parameters
//! - `PresetResolver` — public façade struct
//!
//! ## Dependencies
//! - `edgeforge-core::{Precision, PrecisionPreset}`
//!
//! ## Output
//! `ResolvedPreset` — the concrete parameters for the given preset.
//!
//! ## Preset Table
//! | Preset | Precision | Prefetch | KV Budget |
//! |---|---|---|---|
//! | Quality (default) | FP16 | 1 layer | 8 GiB |
//! | Balanced | Q8 | 1 layer | 6 GiB |
//! | Fast | Q4 | 2 layers | 4 GiB |
//! | Max-Quality | FP32 | 0 layers | 16 GiB |

use edgeforge_core::{Precision, PrecisionPreset};
#[derive(Clone, Debug)]
/// `ResolvedPreset` — see module documentation for purpose and usage.
pub struct ResolvedPreset {
    pub name: PrecisionPreset,
    pub precision: Precision,
    pub prefetch_depth_layers: u32,
    pub kv_cache_budget_gib: u64,
}
/// Resolve — see module documentation for details.
pub fn resolve(p: PrecisionPreset) -> ResolvedPreset {
    match p {
        PrecisionPreset::Quality => ResolvedPreset {
            name: p,
            precision: Precision::Fp16,
            prefetch_depth_layers: 1,
            kv_cache_budget_gib: 8,
        },
        PrecisionPreset::Balanced => ResolvedPreset {
            name: p,
            precision: Precision::Q8,
            prefetch_depth_layers: 1,
            kv_cache_budget_gib: 6,
        },
        PrecisionPreset::Fast => ResolvedPreset {
            name: p,
            precision: Precision::Q4,
            prefetch_depth_layers: 2,
            kv_cache_budget_gib: 4,
        },
        PrecisionPreset::MaxQuality => ResolvedPreset {
            name: p,
            precision: Precision::Fp32,
            prefetch_depth_layers: 0,
            kv_cache_budget_gib: 16,
        },
    }
}
/// `PresetResolver` — see module documentation for purpose and usage.
pub struct PresetResolver;
impl PresetResolver {
    pub fn resolve(p: PrecisionPreset) -> ResolvedPreset {
        resolve(p)
    }
}
