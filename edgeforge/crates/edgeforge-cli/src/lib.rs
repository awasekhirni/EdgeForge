//! # edgeforge-cli — Library Entry Point
//!
//! ## Purpose
//! Exposes the CLI command modules as a library so integration tests can
//! import and test them directly. The binary entry point is in `main.rs`.
//!
//! ## Modules
//! - `commands` — subcommand implementations (serve, model, bench, config)

pub mod commands;
