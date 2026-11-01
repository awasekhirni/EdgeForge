// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! # EdgeForge Tauri Entry Point
//!
//! ## Purpose
//! Binary entry point for the Tauri 2 desktop app. This file is only compiled
//! when the `tauri-shell` feature is enabled on `edgeforge-app`.
//!
//! ## Build
//! ```bash
//! cargo build --release -p edgeforge-app --features tauri-shell
//! ```
//!
//! Or via the Tauri CLI (preferred — handles frontend build + bundling):
//! ```bash
//! cd crates/edgeforge-app/ui
//! npm install
//! npm run tauri dev
//! ```

fn main() {
    // The real Tauri entry point. edgeforge_app::run() with the tauri-shell
    // feature enabled calls tauri::Builder::default().run(...).
    if let Err(e) = edgeforge_app::run() {
        eprintln!("EdgeForge desktop app failed: {e}");
        std::process::exit(1);
    }
}
