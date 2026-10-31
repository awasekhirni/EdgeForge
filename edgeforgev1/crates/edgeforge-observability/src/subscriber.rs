// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use edgeforge_core::Config;
use std::sync::OnceLock;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
pub struct ObservabilityGuard;
static INIT: OnceLock<()> = OnceLock::new();
pub fn init(cfg: &Config) -> ObservabilityGuard {
    INIT.get_or_init(|| {
        let env_filter = EnvFilter::try_new(&cfg.observability.log_level)
            .unwrap_or_else(|_| EnvFilter::new("info"));
        let fmt_layer = fmt::layer().with_target(true);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    });
    ObservabilityGuard
}
