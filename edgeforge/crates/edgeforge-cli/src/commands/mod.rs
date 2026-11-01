//! # CLI Subcommand Definitions
//!
//! ## Purpose
//! Defines the clap argument structs for each subcommand.
//!
//! ## What This File Does
//! - Declares subcommand modules: serve, model, bench, config_cmd
//! - `ServeArgs` — listen address override
//! - `ModelArgs` / `ModelCommand` — List, Import, Verify
//! - `BenchArgs` — model, warmup, tokens
//! - `ConfigArgs` — json flag
//!
//! ## Dependencies
//! - `clap::{Args, Subcommand}`
//!
//! ## Output
//! Argument structs consumed by the subcommand implementations.

pub mod bench;
pub mod config_cmd;
pub mod model;
pub mod run;
pub mod serve;

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
/// `ServeArgs` — see module documentation for purpose and usage.
pub struct ServeArgs {
    #[arg(long)]
    pub listen: Option<String>,
}

#[derive(Args, Debug)]
/// `ModelArgs` — see module documentation for purpose and usage.
pub struct ModelArgs {
    #[command(subcommand)]
    pub command: ModelCommand,
}

#[derive(Subcommand, Debug)]
/// `ModelCommand` enumeration — see module documentation for variants.
pub enum ModelCommand {
    List,
    Import { #[arg(long)] path: String },
    Verify { #[arg(long)] id: String },
}

#[derive(Args, Debug)]
/// `BenchArgs` — see module documentation for purpose and usage.
pub struct BenchArgs {
    #[arg(long)]
    pub model: String,
    #[arg(long, default_value = "32")]
    pub warmup: u32,
    #[arg(long, default_value = "256")]
    pub tokens: u32,
}

#[derive(Args, Debug)]
/// `RunArgs` — arguments for one-shot inference.
pub struct RunArgs {
    /// The model ID to use for inference.
    #[arg(long)]
    pub model: String,
    /// The prompt text.
    #[arg(long)]
    pub prompt: String,
    /// Maximum tokens to generate.
    #[arg(long, default_value = "128")]
    pub max_tokens: u32,
    /// Sampling temperature.
    #[arg(long, default_value = "0.7")]
    pub temperature: f32,
    /// Optional random seed for deterministic generation.
    #[arg(long)]
    pub seed: Option<u64>,
}

#[derive(Args, Debug)]
/// `ConfigArgs` — see module documentation for purpose and usage.
pub struct ConfigArgs {
    #[arg(long)]
    pub json: bool,
}
