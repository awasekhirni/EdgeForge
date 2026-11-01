// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use super::ConfigArgs;
use edgeforge_core::Config;
pub async fn run(args: ConfigArgs, cfg: Config) -> anyhow::Result<()> {
    if args.json {
        println!("{}", serde_json::to_string_pretty(&cfg)?);
    } else {
        println!("{}", toml::to_string(&cfg)?);
    }
    Ok(())
}
