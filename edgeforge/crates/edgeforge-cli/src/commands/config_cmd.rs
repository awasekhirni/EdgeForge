//! # `edgeforge config` — Print Active Configuration
//!
//! ## Purpose
//! Prints the active configuration as JSON or TOML.
//!
//! ## What This File Does
//! - If `--json`: prints `serde_json::to_string_pretty(&cfg)`
//! - Otherwise: prints `toml::to_string(&cfg)`
//!
//! ## Dependencies
//! - `edgeforge-core::Config`
//! - `serde_json`, `toml`
//!
//! ## Output
//! Configuration string printed to stdout.

use super::ConfigArgs;
use edgeforge_core::Config;
/// Executes the main logic of this function.
pub async fn run(args: ConfigArgs, cfg: Config) -> anyhow::Result<()> { if args.json { println!("{}", serde_json::to_string_pretty(&cfg)?); } else { println!("{}", toml::to_string(&cfg)?); } Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_args_json_false() {
        let args = ConfigArgs { json: false };
        assert!(!args.json);
    }

    #[test]
    fn test_config_args_json_true() {
        let args = ConfigArgs { json: true };
        assert!(args.json);
    }
}
