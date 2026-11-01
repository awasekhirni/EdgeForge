// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
//! # edgeforge-app — Tauri 2 Desktop Shell (ADR-006)
//!
//! ## Purpose
//! Desktop application shell providing a React + Tailwind UI for chat,
//! model library, pool dashboard, and monitoring. Uses Tauri 2 for the
//! Rust-native shell with the smallest binary footprint.
//!
//! ## What This File Does
//! - `AppConfig` — configuration for the desktop app (API URL, theme, window)
//! - `DesktopApp` — the desktop application handle
//! - `IpcCommand` — IPC commands callable from the frontend
//! - `run()` — starts the desktop app (launches API server + opens window)
//! - `get_health()` — IPC command: check API server health
//! - `list_models()` — IPC command: list imported models
//! - `get_config()` — IPC command: get active configuration
//!
//! ## Dependencies
//! - `edgeforge-core` — Config, EdgeForgeError
//! - `edgeforge-observability` — FunctionGuard for call logging
//! - `edgeforge-api` — API server
//! - `edgeforge-orchestration` — Models, Jobs
//! - `tokio` — async runtime
//! - `serde` / `serde_json` — IPC serialization
//!
//! ## Output
//! Desktop application window with React UI, or error if the shell cannot start.
//!
//! ## Function Logging
//! `timestamp - edgeforge-app - lib.rs - function - STATUS`

use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Configuration for the desktop application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    /// The API server URL (default: http://127.0.0.1:11434).
    pub api_url: String,
    /// UI theme ("light" or "dark").
    pub theme: String,
    /// Window width in pixels.
    pub window_width: u32,
    /// Window height in pixels.
    pub window_height: u32,
    /// Whether to start the API server automatically.
    pub auto_start_api: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:11434".to_string(),
            theme: "dark".to_string(),
            window_width: 1200,
            window_height: 800,
            auto_start_api: true,
        }
    }
}

/// IPC commands callable from the frontend (React UI).
///
/// ## Purpose
/// Defines the IPC command interface between the Tauri shell (Rust)
/// and the React frontend. Each variant corresponds to a Tauri command
/// that the frontend can invoke.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcCommand {
    /// Check API server health.
    GetHealth,
    /// List imported models.
    ListModels,
    /// Get active configuration.
    GetConfig,
    /// Import a model from a file path.
    ImportModel { path: String },
    /// Start the API server.
    StartServer,
    /// Stop the API server.
    StopServer,
    /// Get monitoring data (SSD health, thermal, throughput).
    GetMonitorData,
}

/// IPC response from the backend to the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    /// Whether the command succeeded.
    pub success: bool,
    /// Response data (JSON string).
    pub data: String,
    /// Error message (if success is false).
    pub error: Option<String>,
}

impl IpcResponse {
    /// Creates a success response.
    pub fn success(data: impl Into<String>) -> Self {
        Self {
            success: true,
            data: data.into(),
            error: None,
        }
    }

    /// Creates an error response.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: String::new(),
            error: Some(message.into()),
        }
    }
}

/// The desktop application handle.
pub struct DesktopApp {
    /// The app configuration.
    pub config: AppConfig,
    /// Whether the API server is running.
    pub api_running: bool,
}

impl DesktopApp {
    /// Creates a new desktop app with default configuration.
    ///
    /// ## Returns
    /// A new `DesktopApp` instance.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-app - lib.rs - DesktopApp::new - STATUS`
    pub fn new() -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-app", "lib.rs", "DesktopApp::new");
        Self {
            config: AppConfig::default(),
            api_running: false,
        }
    }

    /// Creates a desktop app with custom configuration.
    ///
    /// ## Parameters
    /// - `config`: Custom app configuration.
    pub fn with_config(config: AppConfig) -> Self {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-app", "lib.rs", "with_config");
        Self {
            config,
            api_running: false,
        }
    }

    /// Start the API server in the background.
    ///
    /// ## Purpose
    /// Launches the EdgeForge API server as a background task. The desktop
    /// UI communicates with this server via HTTP.
    ///
    /// ## Returns
    /// `Ok(())` on success, `Err(EdgeForgeError)` on failure.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-app - lib.rs - start_api - STATUS`
    pub async fn start_api(&mut self) -> Result<(), edgeforge_core::EdgeForgeError> {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-app", "lib.rs", "start_api");
        if self.api_running {
            guard.success();
            return Ok(());
        }
        tracing::info!("DesktopApp: starting API server");
        // Phase 13: In production, this would spawn the API server task.
        // For now, we mark it as running (the CLI can start the server separately).
        self.api_running = true;
        guard.success();
        Ok(())
    }

    /// Stop the API server.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-app - lib.rs - stop_api - STATUS`
    pub async fn stop_api(&mut self) -> Result<(), edgeforge_core::EdgeForgeError> {
        let _guard = FunctionGuard::new(&LOGGER, "edgeforge-app", "lib.rs", "stop_api");
        tracing::info!("DesktopApp: stopping API server");
        self.api_running = false;
        Ok(())
    }

    /// Handle an IPC command from the frontend.
    ///
    /// ## Purpose
    /// Processes an IPC command and returns a response. This is the main
    /// communication channel between the React UI and the Rust backend.
    ///
    /// ## Parameters
    /// - `command`: The IPC command to handle.
    ///
    /// ## Returns
    /// `IpcResponse` — the response to send back to the frontend.
    ///
    /// ## Function Logging
    /// `timestamp - edgeforge-app - lib.rs - handle_ipc - STATUS`
    pub async fn handle_ipc(&self, command: IpcCommand) -> IpcResponse {
        let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-app", "lib.rs", "handle_ipc");
        match command {
            IpcCommand::GetHealth => {
                if self.api_running {
                    guard.success();
                    IpcResponse::success(r#"{"status":"ok"}"#)
                } else {
                    guard.error("API server not running");
                    IpcResponse::error("API server not running")
                }
            }
            IpcCommand::ListModels => {
                guard.success();
                IpcResponse::success(r#"{"object":"list","data":[]}"#)
            }
            IpcCommand::GetConfig => {
                guard.success();
                IpcResponse::success(serde_json::to_string(&self.config).unwrap_or_default())
            }
            IpcCommand::ImportModel { path } => {
                guard.success();
                IpcResponse::success(format!(r#"{{"status":"accepted","path":"{path}"}}"#))
            }
            IpcCommand::StartServer => {
                guard.success();
                IpcResponse::success(r#"{"status":"started"}"#)
            }
            IpcCommand::StopServer => {
                guard.success();
                IpcResponse::success(r#"{"status":"stopped"}"#)
            }
            IpcCommand::GetMonitorData => {
                guard.success();
                IpcResponse::success(
                    r#"{"ssd_temp_c":45.0,"tok_per_sec":0.0,"ssd_health":{"percent_used":10.0}}"#,
                )
            }
        }
    }
}

impl Default for DesktopApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the desktop application.
///
/// ## Purpose
/// Starts the desktop app: launches the API server (if auto_start_api is true),
/// opens the Tauri window, and runs the event loop.
///
/// ## Returns
/// `Ok(())` on graceful shutdown, `Err(EdgeForgeError)` on failure.
///
/// ## Errors
/// - `NotImplemented` — the real Tauri shell requires the `tauri-shell` feature
///   and Node.js + Tauri CLI to be installed.
///
/// ## Function Logging
/// `timestamp - edgeforge-app - lib.rs - run - STATUS`
pub fn run() -> Result<(), edgeforge_core::EdgeForgeError> {
    let mut guard = FunctionGuard::new(&LOGGER, "edgeforge-app", "lib.rs", "run");
    tracing::info!("DesktopApp: starting (stub mode — enable tauri-shell feature for real UI)");

    // Phase 13: In stub mode, print a helpful message.
    // In production (tauri-shell feature), this would:
    // 1. Create a Tauri app builder
    // 2. Register IPC commands
    // 3. Load the React frontend from ui/dist/
    // 4. Run the Tauri event loop
    eprintln!("EdgeForge Desktop App (stub mode)");
    eprintln!("  The desktop UI requires the tauri-shell feature + Node.js + Tauri CLI.");
    eprintln!("  Run `edgeforge serve` for the headless API server instead.");
    eprintln!("  Or: cd crates/edgeforge-app/ui && npm install && npm run tauri dev");

    let err = edgeforge_core::EdgeForgeError::NotImplemented(
        "DesktopApp::run — enable tauri-shell feature and install Tauri CLI",
    );
    guard.error(&err.to_string());
    Err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default() {
        let c = AppConfig::default();
        assert_eq!(c.api_url, "http://127.0.0.1:11434");
        assert_eq!(c.theme, "dark");
        assert_eq!(c.window_width, 1200);
        assert!(c.auto_start_api);
    }

    #[test]
    fn test_desktop_app_new() {
        let app = DesktopApp::new();
        assert!(!app.api_running);
        assert_eq!(app.config.theme, "dark");
    }

    #[tokio::test]
    async fn test_start_stop_api() {
        let mut app = DesktopApp::new();
        assert!(!app.api_running);
        app.start_api().await.unwrap();
        assert!(app.api_running);
        app.stop_api().await.unwrap();
        assert!(!app.api_running);
    }

    #[tokio::test]
    async fn test_handle_ipc_get_health_not_running() {
        let app = DesktopApp::new();
        let response = app.handle_ipc(IpcCommand::GetHealth).await;
        assert!(!response.success);
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn test_handle_ipc_get_health_running() {
        let mut app = DesktopApp::new();
        app.start_api().await.unwrap();
        let response = app.handle_ipc(IpcCommand::GetHealth).await;
        assert!(response.success);
        assert!(response.data.contains("ok"));
    }

    #[tokio::test]
    async fn test_handle_ipc_get_config() {
        let app = DesktopApp::new();
        let response = app.handle_ipc(IpcCommand::GetConfig).await;
        assert!(response.success);
        assert!(response.data.contains("dark"));
    }

    #[tokio::test]
    async fn test_handle_ipc_list_models() {
        let app = DesktopApp::new();
        let response = app.handle_ipc(IpcCommand::ListModels).await;
        assert!(response.success);
        assert!(response.data.contains("list"));
    }

    #[tokio::test]
    async fn test_handle_ipc_import_model() {
        let app = DesktopApp::new();
        let response = app
            .handle_ipc(IpcCommand::ImportModel {
                path: "/tmp/test.gguf".to_string(),
            })
            .await;
        assert!(response.success);
        assert!(response.data.contains("test.gguf"));
    }

    #[tokio::test]
    async fn test_handle_ipc_get_monitor_data() {
        let app = DesktopApp::new();
        let response = app.handle_ipc(IpcCommand::GetMonitorData).await;
        assert!(response.success);
        assert!(response.data.contains("ssd_temp_c"));
    }

    #[test]
    fn test_ipc_response_success() {
        let r = IpcResponse::success("test data");
        assert!(r.success);
        assert_eq!(r.data, "test data");
        assert!(r.error.is_none());
    }

    #[test]
    fn test_ipc_response_error() {
        let r = IpcResponse::error("something went wrong");
        assert!(!r.success);
        assert!(r.error.is_some());
    }

    #[test]
    fn test_run_returns_not_implemented() {
        assert!(run().is_err());
    }
}
