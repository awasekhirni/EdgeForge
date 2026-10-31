// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
#[derive(Clone, Debug)]
pub struct VramTierConfig {
    pub enabled: bool,
    pub budget_bytes: u64,
    pub backend: VramBackend,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VramBackend {
    AppleUnified,
    Cuda,
    Rocm,
    None,
}
pub struct VramTier {
    config: VramTierConfig,
}
impl VramTier {
    pub fn new(config: VramTierConfig) -> Self {
        Self { config }
    }
    pub fn enabled(&self) -> bool {
        self.config.enabled && self.config.backend != VramBackend::None
    }
}
