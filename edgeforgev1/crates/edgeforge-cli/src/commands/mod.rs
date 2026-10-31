// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
pub mod bench;
pub mod config_cmd;
pub mod model;
pub mod serve;

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(long)]
    pub listen: Option<String>,
}

#[derive(Args, Debug)]
pub struct ModelArgs {
    #[command(subcommand)]
    pub command: ModelCommand,
}

#[derive(Subcommand, Debug)]
pub enum ModelCommand {
    List,
    Import {
        #[arg(long)]
        path: String,
    },
    Verify {
        #[arg(long)]
        id: String,
    },
}

#[derive(Args, Debug)]
pub struct BenchArgs {
    #[arg(long)]
    pub model: String,
    #[arg(long, default_value = "32")]
    pub warmup: u32,
    #[arg(long, default_value = "256")]
    pub tokens: u32,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[arg(long)]
    pub json: bool,
}
