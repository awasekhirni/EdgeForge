// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::{Precision, PrecisionPreset};
#[derive(Clone, Debug)]
pub struct ResolvedPreset {
    pub name: PrecisionPreset,
    pub precision: Precision,
    pub prefetch_depth_layers: u32,
    pub kv_cache_budget_gib: u64,
}
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
pub struct PresetResolver;
impl PresetResolver {
    pub fn resolve(p: PrecisionPreset) -> ResolvedPreset {
        resolve(p)
    }
}
