// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # `edgeforge serve` — Start the API Server
//!
//! ## Purpose
//! Opens the SQLite database, applies migrations, creates AppState, and starts
//! the axum API server.
//!
//! ## What This File Does
//! - Resolves the listen address (from args or config)
//! - Opens SQLite database at `data_dir/edgeforge.db`
//! - Runs all 16 SQLx migrations
//! - Creates AppState with Models + Jobs
//! - Calls `edgeforge_api::serve(addr, state)`
//!
//! ## Dependencies
//! - `edgeforge-api::{AppState, serve}` — API server
//! - `edgeforge-orchestration::{Models, Jobs}` — ORM
//! - `sqlx::sqlite::SqlitePoolOptions` — connection pool
//! - `sqlx::migrate!` — migration runner
//!
//! ## Output
//! Runs the API server until Ctrl+C or shutdown.
//!
//! ## Function: run
//! - **Purpose:** Start the API server
//! - **Parameters:** `ServeArgs` (listen override), `Config`
//! - **Returns:** `anyhow::Result<()>`
//! - **Side effects:** Creates `data_dir` if it doesn't exist; opens SQLite DB;
//!   applies migrations; binds TCP listener

use super::ServeArgs;
use edgeforge_api::AppState;
use edgeforge_core::Config;
use edgeforge_orchestration::{Jobs, Models};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
/// Executes the main logic of this function.
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
