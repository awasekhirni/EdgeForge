//! # Heartbeat-Driven Recovery (F-NET-06)
//!
//! ## Purpose
//! Heartbeat-based peer liveness monitoring. On timeout, marks the peer's
//! experts as remote-fetch-failed and re-pages from local SSD or an alternate peer.
//!
//! ## What This File Does
//! - `HeartbeatConfig` — interval, timeout, miss threshold
//! - `Heartbeat` — per-peer heartbeat state
//! - `Heartbeat::on_received()` — record a received heartbeat
//! - `Heartbeat::on_missed()` — record a missed heartbeat; returns true if peer should die
//! - `repage_from_ssd(weight_pager, layer, expert)` — re-page an expert from
//!   local SSD after peer loss, using the real `WeightPager`
//! - `repage_from_ssd_simple(expert_id)` — convenience that parses a
//!   "layer_N_expert_M" string and delegates to a temp-file WeightPager
//!
//! ## Dependencies
//! - `edgeforge-core` — EdgeForgeError, LayerIdx, ExpertId, Bytes
//! - `edgeforge-storage` — `WeightPager`, `WeightPagerConfig`, `WeightPageId`, `PlatformIo`, `BlockingIo`
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `std::time::Duration` — heartbeat intervals

use edgeforge_core::{Bytes, EdgeForgeError, ExpertId, LayerIdx};
use edgeforge_observability::function_logger::FunctionGuard;
use std::sync::Arc;
use std::time::Duration;

/// Global logger reference for this module.
static LOGGER: std::sync::LazyLock<edgeforge_observability::FunctionLogger> =
    std::sync::LazyLock::new(edgeforge_observability::FunctionLogger::new);

/// Heartbeat configuration.
#[derive(Clone, Copy, Debug)]
pub struct HeartbeatConfig {
    /// How often to send heartbeats.
    pub interval: Duration,
    /// How long to wait before considering a heartbeat missed.
    pub timeout: Duration,
    /// Number of consecutive missed heartbeats before declaring peer dead.
    pub miss_threshold: u8,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            timeout: Duration::from_secs(15),
            miss_threshold: 3,
        }
    }
}

/// Per-peer heartbeat state.
///
/// ## Purpose
/// Tracks the number of missed heartbeats for a single peer. When the
/// miss count reaches the threshold, the peer is declared dead.
pub struct Heartbeat {
    /// The heartbeat configuration.
    pub config: HeartbeatConfig,
    /// Number of consecutive missed heartbeats.
    pub missed_count: u8,
    /// Timestamp of the last received heartbeat (Unix ms).
    pub last_received_ms: i64,
    /// Whether the peer is alive.
    pub alive: bool,
}

impl Heartbeat {
    /// Creates a new heartbeat tracker with the given configuration.
    pub fn new(config: HeartbeatConfig) -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "Heartbeat::new");
        Self {
            config,
            missed_count: 0,
            last_received_ms: 0,
            alive: true,
        }
    }

    /// Record a received heartbeat.
    pub fn on_received(&mut self, timestamp_ms: i64) {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "on_received");
        self.missed_count = 0;
        self.last_received_ms = timestamp_ms;
        self.alive = true;
        tracing::trace!(timestamp_ms, "Heartbeat received");
    }

    /// Record a missed heartbeat.
    ///
    /// ## Returns
    /// `true` if the peer should be considered dead (miss threshold reached).
    pub fn on_missed(&mut self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "on_missed");
        self.missed_count += 1;
        tracing::warn!(missed = self.missed_count, threshold = self.config.miss_threshold, "Heartbeat missed");
        if self.missed_count >= self.config.miss_threshold {
            self.alive = false;
            tracing::warn!("Peer declared dead after {} missed heartbeats", self.missed_count);
            return true;
        }
        false
    }

    /// Returns whether the peer is alive.
    pub fn is_alive(&self) -> bool {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "is_alive");
        self.alive
    }

    /// Returns the number of consecutive missed heartbeats.
    pub fn missed_count(&self) -> u8 {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "missed_count");
        self.missed_count
    }

    /// Returns the time since the last received heartbeat.
    pub fn time_since_last(&self, now_ms: i64) -> Duration {
        let _guard =
            FunctionGuard::new(&LOGGER, "edgeforge-net", "recovery.rs", "time_since_last");
        if self.last_received_ms == 0 {
            return Duration::MAX;
        }
        Duration::from_millis((now_ms - self.last_received_ms).max(0) as u64)
    }
}

/// Re-page an expert from local SSD after a peer is lost.
///
/// ## Purpose
/// When a peer is declared dead, its experts need to be re-paged from local
/// SSD. This function calls the real `WeightPager` to read the expert's
/// weight page from disk.
///
/// ## Parameters
/// - `weight_pager`: The weight pager to use for the SSD read.
/// - `layer`: The layer index of the expert to re-page.
/// - `expert`: The expert ID to re-page.
///
/// ## Returns
/// `Ok(Vec<u8>)` — the weight page bytes read from SSD.
///
/// ## Errors
/// - `Io` if the SSD read fails.
/// - `Engine` if the weight pager fails to open the file.
///
/// ## Dependencies
/// - `edgeforge_storage::WeightPager` — for the actual SSD read.
pub async fn repage_from_ssd(
    weight_pager: &edgeforge_storage::WeightPager,
    layer: LayerIdx,
    expert: ExpertId,
) -> Result<Vec<u8>, EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "recovery.rs",
        "repage_from_ssd",
    );

    tracing::info!(
        layer = layer.0,
        expert = expert.0,
        "Re-paging expert from local SSD after peer loss"
    );

    let page_id = edgeforge_storage::weight_pager::WeightPageId {
        layer,
        expert: Some(expert),
        page: 0,
    };

    let page = weight_pager.page_in(page_id).await?;

    tracing::info!(
        layer = layer.0,
        expert = expert.0,
        bytes = page.bytes.len(),
        "Expert re-paged from local SSD"
    );

    guard.success();
    Ok(page.bytes)
}

/// Re-page an expert from a temp-file-backed SSD (convenience for tests).
///
/// ## Purpose
/// Convenience function that creates a temp file, writes `weight_bytes` to
/// it, builds a `WeightPager` over the temp file, and re-pages the expert.
/// Used by tests and by callers that don't have a pre-configured WeightPager.
///
/// ## Parameters
/// - `expert_id`: A string like "layer_3_expert_5" identifying the expert.
/// - `weight_bytes`: The weight bytes to write to the temp file.
///
/// ## Returns
/// `Ok(Vec<u8>)` — the weight bytes read back from the temp file.
///
/// ## Errors
/// - `InvalidRequest` if `expert_id` doesn't match the `layer_N_expert_M` format.
/// - `Io` if the temp file cannot be created or read.
pub async fn repage_from_ssd_simple(
    expert_id: &str,
    weight_bytes: &[u8],
) -> Result<Vec<u8>, EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "recovery.rs",
        "repage_from_ssd_simple",
    );

    // Parse "layer_N_expert_M" → (LayerIdx(N), ExpertId(M)).
    let (layer, expert) = parse_expert_id(expert_id)?;

    // Create a temp file and write the weight bytes to it.
    let tmp = tempfile::NamedTempFile::new().map_err(|e| EdgeForgeError::Io {
        path: std::path::PathBuf::from("<tempfile>"),
        source: e,
    })?;
    std::fs::write(tmp.path(), weight_bytes).map_err(|e| EdgeForgeError::Io {
        path: tmp.path().to_path_buf(),
        source: e,
    })?;

    // Build a WeightPager over the temp file.
    let io: Arc<dyn edgeforge_storage::PlatformIo> = Arc::new(edgeforge_storage::BlockingIo::new());
    let pager = edgeforge_storage::WeightPager::new(
        edgeforge_storage::weight_pager::WeightPagerConfig {
            page_size: Bytes(weight_bytes.len() as u64),
            ram_budget: Bytes(1024 * 1024),
            ssd_path: tmp.path().to_path_buf(),
        },
        io,
        edgeforge_core::ModelId::new(),
    );

    // Re-page.
    let result = repage_from_ssd(&pager, layer, expert).await?;
    guard.success();
    Ok(result)
}

/// Parse an expert ID string of the form "layer_N_expert_M".
///
/// ## Parameters
/// - `s`: The string to parse (e.g., "layer_3_expert_5").
///
/// ## Returns
/// `Ok((LayerIdx(N), ExpertId(M)))` on success.
///
/// ## Errors
/// - `InvalidRequest` if the string doesn't match the expected format.
fn parse_expert_id(s: &str) -> Result<(LayerIdx, ExpertId), EdgeForgeError> {
    let mut guard = FunctionGuard::new(
        &LOGGER,
        "edgeforge-net",
        "recovery.rs",
        "parse_expert_id",
    );

    // Expected format: "layer_<u32>_expert_<u32>"
    let parts: Vec<&str> = s.split('_').collect();
    if parts.len() != 4 || parts[0] != "layer" || parts[2] != "expert" {
        let msg = format!(
            "invalid expert_id '{s}': expected 'layer_<N>_expert_<M>'"
        );
        guard.error(&msg);
        return Err(EdgeForgeError::InvalidRequest(msg));
    }

    let layer_n: u32 = parts[1].parse().map_err(|_| {
        let msg = format!("invalid layer number in expert_id '{s}'");
        EdgeForgeError::InvalidRequest(msg)
    })?;
    let expert_n: u32 = parts[3].parse().map_err(|_| {
        let msg = format!("invalid expert number in expert_id '{s}'");
        EdgeForgeError::InvalidRequest(msg)
    })?;

    guard.success();
    Ok((LayerIdx(layer_n), ExpertId(expert_n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_received_resets_counter() {
        let mut hb = Heartbeat::new(HeartbeatConfig::default());
        hb.on_missed();
        hb.on_missed();
        assert_eq!(hb.missed_count(), 2);
        hb.on_received(1000);
        assert_eq!(hb.missed_count(), 0);
        assert!(hb.is_alive());
    }

    #[test]
    fn test_heartbeat_missed_reaches_threshold() {
        let mut hb = Heartbeat::new(HeartbeatConfig {
            miss_threshold: 3,
            ..Default::default()
        });
        assert!(!hb.on_missed()); // 1
        assert!(!hb.on_missed()); // 2
        assert!(hb.on_missed()); // 3 → dead
        assert!(!hb.is_alive());
    }

    #[test]
    fn test_heartbeat_alive_initially() {
        let hb = Heartbeat::new(HeartbeatConfig::default());
        assert!(hb.is_alive());
        assert_eq!(hb.missed_count(), 0);
    }

    #[test]
    fn test_time_since_last_never_received() {
        let hb = Heartbeat::new(HeartbeatConfig::default());
        assert_eq!(hb.time_since_last(1000), Duration::MAX);
    }

    #[test]
    fn test_time_since_last_received() {
        let mut hb = Heartbeat::new(HeartbeatConfig::default());
        hb.on_received(1000);
        let elapsed = hb.time_since_last(1500);
        assert_eq!(elapsed.as_millis(), 500);
    }

    #[test]
    fn test_heartbeat_revives_after_death() {
        let mut hb = Heartbeat::new(HeartbeatConfig {
            miss_threshold: 2,
            ..Default::default()
        });
        assert!(!hb.on_missed()); // 1
        assert!(hb.on_missed()); // 2 → dead
        assert!(!hb.is_alive());
        // A received heartbeat should revive the peer.
        hb.on_received(2000);
        assert!(hb.is_alive());
        assert_eq!(hb.missed_count(), 0);
    }

    // ── repage_from_ssd tests ──

    #[tokio::test]
    async fn test_repage_from_ssd_simple_succeeds() {
        // Previously: asserted is_err(). Now: must succeed.
        let weights = vec![0xABu8; 4096];
        let result = repage_from_ssd_simple("layer_3_expert_5", &weights).await;
        assert!(result.is_ok(), "repage_from_ssd_simple should succeed: {result:?}");
        let paged = result.unwrap();
        assert_eq!(paged.len(), 4096);
        assert_eq!(paged[0], 0xAB);
    }

    #[tokio::test]
    async fn test_repage_from_ssd_simple_returns_correct_bytes() {
        let weights = vec![0xCDu8; 8192];
        let paged = repage_from_ssd_simple("layer_0_expert_0", &weights)
            .await
            .unwrap();
        assert_eq!(paged, weights);
    }

    #[tokio::test]
    async fn test_repage_from_ssd_simple_invalid_id() {
        let result = repage_from_ssd_simple("not_a_valid_expert_id", &[0u8; 16]).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(EdgeForgeError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn test_repage_from_ssd_simple_invalid_layer_number() {
        let result = repage_from_ssd_simple("layer_abc_expert_5", &[0u8; 16]).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(EdgeForgeError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn test_repage_from_ssd_simple_invalid_expert_number() {
        let result = repage_from_ssd_simple("layer_3_expert_xyz", &[0u8; 16]).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(EdgeForgeError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn test_repage_from_ssd_with_real_weight_pager() {
        use edgeforge_storage::weight_pager::{WeightPageId, WeightPager, WeightPagerConfig};

        // Set up a temp file with enough bytes for layer 5 * 4096-byte pages.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let data: Vec<u8> = (0..(6 * 4096)).map(|i| (i % 256) as u8).collect();
        std::fs::write(tmp.path(), &data).unwrap();

        let io: Arc<dyn edgeforge_storage::PlatformIo> =
            Arc::new(edgeforge_storage::BlockingIo::new());
        let pager = WeightPager::new(
            WeightPagerConfig {
                page_size: Bytes(4096),
                ram_budget: Bytes(1024 * 1024),
                ssd_path: tmp.path().to_path_buf(),
            },
            io,
            edgeforge_core::ModelId::new(),
        );

        // Re-page layer 5, expert 7.
        let paged = repage_from_ssd(&pager, LayerIdx(5), ExpertId(7))
            .await
            .expect("repage_from_ssd should succeed");
        assert_eq!(paged.len(), 4096);
        // Layer 5's page starts at offset 5 * 4096 = 20480.
        assert_eq!(paged[0], (20480 % 256) as u8);
    }

    // ── parse_expert_id tests ──

    #[test]
    fn test_parse_expert_id_valid() {
        let (layer, expert) = parse_expert_id("layer_3_expert_5").unwrap();
        assert_eq!(layer, LayerIdx(3));
        assert_eq!(expert, ExpertId(5));
    }

    #[test]
    fn test_parse_expert_id_zero() {
        let (layer, expert) = parse_expert_id("layer_0_expert_0").unwrap();
        assert_eq!(layer, LayerIdx(0));
        assert_eq!(expert, ExpertId(0));
    }

    #[test]
    fn test_parse_expert_id_large_numbers() {
        let (layer, expert) = parse_expert_id("layer_4294967295_expert_4294967295").unwrap();
        assert_eq!(layer, LayerIdx(u32::MAX));
        assert_eq!(expert, ExpertId(u32::MAX));
    }

    #[test]
    fn test_parse_expert_id_missing_layer_prefix() {
        assert!(parse_expert_id("3_expert_5").is_err());
    }

    #[test]
    fn test_parse_expert_id_missing_expert_prefix() {
        assert!(parse_expert_id("layer_3_5").is_err());
    }

    #[test]
    fn test_parse_expert_id_extra_parts() {
        assert!(parse_expert_id("layer_3_expert_5_extra").is_err());
    }
}
