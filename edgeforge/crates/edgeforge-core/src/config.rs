// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # Strongly-Typed Configuration
//!
//! ## Purpose
//! Loads configuration from TOML files and environment variables with full
//! type safety. Defaults honour ADR-008 (air-gapped) and ADR-002 (2 TB NVMe).
//!
//! ## What This File Does
//! - Defines `Config` struct with sections: network, storage, inference, api,
//!   observability, updates, paths
//! - `Config::load()` — merges defaults → TOML file → env vars
//! - `Config::validate()` — sanity-checks the config at startup
//! - Defines `Precision` enum (FP32/FP16/BF16/Q8/Q4/Q3) with `bytes_per_param()`
//! - Defines `PrecisionPreset` enum (Quality/Balanced/Fast/MaxQuality)
//! - Defines `NetworkMode` (AirGapped/LanOpen/GlobalRelay)
//! - Defines `Role` and `ChatMessage` for OpenAI-compatible chat
//!
//! ## Dependencies
//! - `figment` — config loading (TOML + env providers)
//! - `serde` — Serialize/Deserialize
//!
//! ## Output
//! `Config` struct with all settings, validated at startup.
//!
//! ## Config Search Path
//! 1. Built-in defaults
//! 2. `~/.config/edgeforge/config.toml` (or `EDGEFORGE_CONFIG`)
//! 3. Environment variables prefixed `EDGEFORGE_` (e.g. `EDGEFORGE_NETWORK__MODE`)

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `Config` — see module documentation for purpose and usage.
pub struct Config {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
    pub inference: InferenceConfig,
    pub api: ApiConfig,
    pub observability: ObservabilityConfig,
    pub updates: UpdatesConfig,
    pub paths: PathsConfig,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            network: Default::default(),
            storage: Default::default(),
            inference: Default::default(),
            api: Default::default(),
            observability: Default::default(),
            updates: Default::default(),
            paths: Default::default(),
        }
    }
}
impl Config {
    /// Loads a resource from storage.
    pub fn load() -> Result<Self, figment::Error> {
        let path = Self::default_path();
        let mut fig = Figment::from(Serialized::defaults(Self::default()));
        if path.exists() {
            fig = fig.merge(Toml::file(path));
        }
        fig = fig.merge(Env::prefixed("EDGEFORGE_").split("__"));
        fig.extract()
    }
    /// Default path — see module documentation for details.
    pub fn default_path() -> PathBuf {
        if let Ok(p) = std::env::var("EDGEFORGE_CONFIG") {
            return PathBuf::from(p);
        }
        let mut base = if let Ok(d) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(d)
        } else if let Ok(h) = std::env::var("HOME") {
            PathBuf::from(&h).join(".config")
        } else {
            PathBuf::from(".")
        };
        base.push("edgeforge");
        base.push("config.toml");
        base
    }
    /// Validate — see module documentation for details.
    pub fn validate(&self) -> Result<(), String> {
        if self.storage.min_ssd_gib < 2 * 1024 {
            return Err(format!("min_ssd_gib must be >= {}", 2 * 1024));
        }
        if self.observability.otlp_enabled && self.observability.otlp_endpoint.is_none() {
            return Err("otlp_endpoint required".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `NetworkConfig` — see module documentation for purpose and usage.
pub struct NetworkConfig {
    pub mode: NetworkMode,
    pub telemetry: bool,
    pub pool_listen_port: u16,
    pub pool_allowed_peers: Vec<String>,
}
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: NetworkMode::AirGapped,
            telemetry: false,
            pool_listen_port: 11435,
            pool_allowed_peers: Vec::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `NetworkMode` enumeration — see module documentation for variants.
pub enum NetworkMode {
    AirGapped,
    LanOpen,
    GlobalRelay,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `StorageConfig` — see module documentation for purpose and usage.
pub struct StorageConfig {
    pub min_ssd_gib: u64,
    pub default_precision: Precision,
    pub prefetch_depth_layers: u32,
    pub kv_cache_budget_gib: u64,
    pub kv_page_size_kib: u32,
    pub use_direct_io: bool,
    pub thermal_poll_interval_secs: u32,
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            min_ssd_gib: 2 * 1024,
            default_precision: Precision::Fp16,
            prefetch_depth_layers: 1,
            kv_cache_budget_gib: 8,
            kv_page_size_kib: 4 * 1024,
            use_direct_io: true,
            thermal_poll_interval_secs: 5,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `Precision` enumeration — see module documentation for variants.
pub enum Precision {
    Fp32,
    Fp16,
    Bf16,
    Q8,
    Q4,
    Q3,
}
impl Precision {
    pub const fn bytes_per_param(self) -> f32 {
        match self {
            Self::Fp32 => 4.0,
            Self::Fp16 | Self::Bf16 => 2.0,
            Self::Q8 => 1.0,
            Self::Q4 => 0.5,
            Self::Q3 => 0.375,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `PrecisionPreset` enumeration — see module documentation for variants.
pub enum PrecisionPreset {
    Quality,
    Balanced,
    Fast,
    MaxQuality,
}
impl PrecisionPreset {
    pub const fn precision(self) -> Precision {
        match self {
            Self::Quality => Precision::Fp16,
            Self::Balanced => Precision::Q8,
            Self::Fast => Precision::Q4,
            Self::MaxQuality => Precision::Fp32,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `InferenceConfig` — see module documentation for purpose and usage.
pub struct InferenceConfig {
    pub interactive_slots: u32,
    pub batch_slots: u32,
    pub continuous_batching: bool,
    pub determinism: bool,
}
impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            interactive_slots: 1,
            batch_slots: 1,
            continuous_batching: false,
            determinism: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `ApiConfig` — see module documentation for purpose and usage.
pub struct ApiConfig {
    pub listen_addr: String,
    pub require_api_key: bool,
    pub rate_limit_per_minute: u32,
}
impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:11434".into(),
            require_api_key: false,
            rate_limit_per_minute: 600,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `ObservabilityConfig` — see module documentation for purpose and usage.
pub struct ObservabilityConfig {
    pub otlp_enabled: bool,
    pub otlp_endpoint: Option<String>,
    pub log_level: String,
}
impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            otlp_enabled: false,
            otlp_endpoint: None,
            log_level: "info".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `UpdatesConfig` — see module documentation for purpose and usage.
pub struct UpdatesConfig {
    pub channel: ReleaseChannel,
    pub auto_install: bool,
}
impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            channel: ReleaseChannel::Stable,
            auto_install: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// `ReleaseChannel` enumeration — see module documentation for variants.
pub enum ReleaseChannel {
    Stable,
    Beta,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// `PathsConfig` — see module documentation for purpose and usage.
pub struct PathsConfig {
    pub data_dir: PathBuf,
    pub model_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
}
impl Default for PathsConfig {
    fn default() -> Self {
        let d = if let Ok(v) = std::env::var("EDGEFORGE_DATA") {
            PathBuf::from(v)
        } else if let Ok(h) = std::env::var("HOME") {
            PathBuf::from(&h).join(".local/share/edgeforge")
        } else {
            PathBuf::from(".edgeforge")
        };
        Self {
            model_dir: d.join("models"),
            cache_dir: d.join("cache"),
            log_dir: d.join("logs"),
            data_dir: d,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
/// `ChatMessage` — see module documentation for purpose and usage.
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// `Role` enumeration — see module documentation for variants.
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}
impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults() {
        let c = Config::default();
        assert_eq!(c.network.mode, NetworkMode::AirGapped);
        assert_eq!(c.storage.default_precision, Precision::Fp16);
        assert_eq!(c.storage.min_ssd_gib, 2 * 1024);
    }
    #[test]
    fn preset_maps() {
        assert_eq!(PrecisionPreset::Quality.precision(), Precision::Fp16);
    }
    #[test]
    fn validation() {
        let mut c = Config::default();
        c.storage.min_ssd_gib = 512;
        assert!(c.validate().is_err());
    }
}
