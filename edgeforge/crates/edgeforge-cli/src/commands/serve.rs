//! # `edgeforge serve` — Start the API Server
//!
//! ## Purpose
//! Opens the SQLite database, applies migrations, creates AppState, and starts
//! the axum API server with the full middleware stack (auth + rate limiting +
//! logging + tracing).
//!
//! ## What This File Does
//! - Resolves the listen address (from args or config)
//! - Opens SQLite database at `data_dir/edgeforge.db`
//! - Runs all SQLx migrations (0001–0018)
//! - Creates AppState with Models + Jobs + Embeddings + HashingEmbedder +
//!   auth config + rate limiter
//! - Calls `edgeforge_api::serve(addr, state)`
//!
//! ## Auth Configuration
//! The API key is read from `cfg.api.api_key` (set in `config.toml` or via
//! the `EDGEFORGE_API__API_KEY` env var). When `require_api_key` is true and
//! `api_key` is set, requests must include `Authorization: Bearer <api_key>`.
//! When `require_api_key` is true but `api_key` is None, any bearer token is
//! accepted (useful for testing).
//!
//! ## Dependencies
//! - `edgeforge-api::{AppState, serve}` — API server
//! - `edgeforge-orchestration::{Models, Jobs, Embeddings}` — ORM
//! - `edgeforge-runtime::HashingEmbedder` — default embedding engine
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
use edgeforge_orchestration::{Embeddings, Jobs, Models};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;

/// Executes the main logic of this function.
pub async fn run(args: ServeArgs, cfg: Config) -> anyhow::Result<()> {
    let addr_str = args.listen.unwrap_or_else(|| cfg.api.listen_addr.clone());
    let addr: SocketAddr = addr_str.parse().map_err(|e| anyhow::anyhow!("invalid addr: {e}"))?;
    let db_path = cfg.paths.data_dir.join("edgeforge.db");
    if let Some(p) = db_path.parent() { std::fs::create_dir_all(p)?; }
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new().max_connections(8).connect(&db_url).await?;
    sqlx::migrate!("../../resources/migrations").run(&pool).await?;

    // Read the API key from config (set via config.toml or EDGEFORGE_API__API_KEY env var).
    // Also allow the EDGEFORGE_API_KEY env var as a convenience.
    let api_key = cfg
        .api
        .api_key
        .clone()
        .or_else(|| std::env::var("EDGEFORGE_API_KEY").ok());

    tracing::info!(
        require_api_key = cfg.api.require_api_key,
        has_api_key = api_key.is_some(),
        rate_limit_per_minute = cfg.api.rate_limit_per_minute,
        "serve: auth configuration"
    );

    let state = AppState::new(
        Models::new(pool.clone()),
        Jobs::new(pool.clone()),
        Embeddings::new(pool),
        cfg.api.require_api_key,
        api_key,
        cfg.api.rate_limit_per_minute,
    );
    tracing::info!(
        embedder_model_id = %state.embedder.model_id(),
        embedder_dim = state.embedder.dim(),
        "serve: embedder configured"
    );
    edgeforge_api::serve(addr, state).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_args_default_listen_is_none() {
        let args = ServeArgs { listen: None };
        assert!(args.listen.is_none());
    }

    #[test]
    fn test_serve_args_custom_listen() {
        let args = ServeArgs { listen: Some("0.0.0.0:8080".into()) };
        assert_eq!(args.listen.as_deref(), Some("0.0.0.0:8080"));
    }
}
