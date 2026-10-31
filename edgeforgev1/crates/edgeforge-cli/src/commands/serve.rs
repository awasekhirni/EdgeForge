// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use super::ServeArgs;
use edgeforge_api::AppState;
use edgeforge_core::Config;
use edgeforge_orchestration::{Jobs, Models};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
pub async fn run(args: ServeArgs, cfg: Config) -> anyhow::Result<()> {
    let addr_str = args.listen.unwrap_or_else(|| cfg.api.listen_addr.clone());
    let addr: SocketAddr = addr_str
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid addr: {e}"))?;
    let db_path = cfg.paths.data_dir.join("edgeforge.db");
    if let Some(p) = db_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await?;
    sqlx::migrate!("../../resources/migrations")
        .run(&pool)
        .await?;
    let state = AppState::new(
        Models::new(pool.clone()),
        Jobs::new(pool),
        cfg.api.require_api_key,
        cfg.api.rate_limit_per_minute,
    );
    edgeforge_api::serve(addr, state).await?;
    Ok(())
}
