// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge CLI Binary
//!
//! ## Purpose
//! Command-line interface for EdgeForge. Subcommands: serve, model, bench, config.
//!
//! ## What This File Does
//! - Parses CLI args via clap derive
//! - Loads Config (TOML + env vars)
//! - Validates config at startup
//! - Initialises observability (tracing)
//! - Dispatches to the appropriate subcommand
//!
//! ## Dependencies
//! - `clap` — CLI argument parsing
//! - `edgeforge-core::Config` — configuration
//! - `edgeforge-observability` — tracing init
//! - `commands::{serve, model, bench, config_cmd}` — subcommand implementations
//!
//! ## Output
//! ExitCode::SUCCESS (0) on success, ExitCode::FAILURE (1) on error.
//!
//! ## Subcommands
//! | Command | Description |
//! |---|---|
//! | `serve` | Start the OpenAI-compatible API server |
//! | `model list` | List imported models |
//! | `model import --path <p>` | Import a model |
//! | `model verify --id <id>` | Verify a model's integrity |
//! | `bench --model <id>` | Run a throughput benchmark |
//! | `config --json` | Print active configuration |

mod commands;
use clap::{Parser, Subcommand};
use edgeforge_core::Config;
use std::process::ExitCode;
#[derive(Parser, Debug)]
#[command(
    name = "edgeforge",
    version,
    about = "Local-first SSD-backed runtime for frontier-scale AI models"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug)]
enum Command {
    Serve(commands::ServeArgs),
    Model(commands::ModelArgs),
    Bench(commands::BenchArgs),
    Config(commands::ConfigArgs),
}
#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: config: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = cfg.validate() {
        eprintln!("error: invalid config: {e}");
        return ExitCode::FAILURE;
    }
    let _guard = edgeforge_observability::init(&cfg);
    let result = match cli.command {
        Command::Serve(a) => commands::serve::run(a, cfg).await,
        Command::Model(a) => commands::model::run(a, cfg).await,
        Command::Bench(a) => commands::bench::run(a, cfg).await,
        Command::Config(a) => commands::config_cmd::run(a, cfg).await,
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
