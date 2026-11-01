# EdgeForge — Phase 20: Tauri Desktop App Setup Guide

**Version:** 20.0
**Date:** 2026-11-01

This guide covers the EdgeForge desktop application: headless mode, Tauri
mode, the React UI, and the IPC command interface.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: Headless Mode (default)](#quick-start-headless-mode)
3. [Quick Start: Tauri Mode (real window)](#quick-start-tauri-mode)
4. [React UI Development](#react-ui-development)
5. [IPC Command Reference](#ipc-command-reference)
6. [Configuration](#configuration)
7. [Building a Distributable Bundle](#building-a-distributable-bundle)
8. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│  Tauri Window (native webview) — only in tauri-shell mode   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  React UI (ui/src/)                                   │  │
│  │  App.jsx → ChatPanel, ModelPanel, MonitorPanel,       │  │
│  │            HealthBar                                  │  │
│  └───────────────────────────────────────────────────────┘  │
│                             │ HTTP (fetch)                  │
│                             ▼                                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  edgeforge-app (Rust)                                 │  │
│  │  DesktopApp::handle_ipc() → reqwest → API server      │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │ HTTP
                             ▼
┌─────────────────────────────────────────────────────────────┐
│  EdgeForge API Server (edgeforge serve)                     │
│  GET /health, GET/POST /v1/*                                 │
└─────────────────────────────────────────────────────────────┘
```

### Two Modes

| Mode | Feature Flag | Window | System Deps | Use Case |
|---|---|---|---|---|
| **Headless** (default) | (none) | No | None | CI, servers, dev |
| **Tauri** | `tauri-shell` | Yes (native) | webkit2gtk / WebView2 / WKWebView | Desktop app |

**Headless mode never returns `NotImplemented`** — it's a real, working
implementation that runs the IPC service without opening a window.

---

## Quick Start: Headless Mode

The default build runs in headless mode — no window, no system webview
dependencies. The IPC service is active and the React UI can be served
separately.

### Build + run

```bash
cargo build --release -p edgeforge-app
./target/release/edgeforge-app
```

Output:
```
EdgeForge Desktop App (headless mode)
========================================

The desktop UI is not bundled in this build.

To use the full desktop UI:
  1. Build with the tauri-shell feature:
       cargo build --release -p edgeforge-app --features tauri-shell

  2. Or run the React UI separately + the API server:
       ./target/release/edgeforge serve &
       cd crates/edgeforge-app/ui && npm install && npm run dev
       # Open http://localhost:5173 in your browser
```

### Run the UI separately (headless + browser)

```bash
# Terminal 1: API server
./target/release/edgeforge serve &

# Terminal 2: React UI (Vite dev server)
cd crates/edgeforge-app/ui
npm install
npm run dev
# Open http://localhost:5173
```

The React UI talks directly to the API server via HTTP — no Tauri required.

---

## Quick Start: Tauri Mode

Tauri mode opens a real native window with the React UI loaded from
`ui/dist/`.

### Prerequisites

- **Linux**: `webkit2gtk-4.1-dev`, `libgtk-3-dev`, `libappindicator3-dev`
- **macOS**: Xcode + Command Line Tools
- **Windows**: WebView2 (preinstalled on Win10/11)

### Install UI dependencies

```bash
cd crates/edgeforge-app/ui
npm install
```

### Dev mode (hot reload)

```bash
npm run tauri dev
```

This starts the Vite dev server + the Tauri shell. The window opens
automatically. Edits to the React UI hot-reload instantly.

### Production build

```bash
npm run tauri build
```

Output bundles are in `crates/edgeforge-app/src-tauri/target/release/bundle/`:
- Linux: `.deb`, `.AppImage`
- macOS: `.dmg`, `.app`
- Windows: `.msi`, `.exe` (NSIS)

---

## React UI Development

### Directory structure

```
crates/edgeforge-app/ui/
├── index.html              # Vite entry point
├── package.json            # npm deps + scripts
├── postcss.config.js       # PostCSS (Tailwind + Autoprefixer)
├── tailwind.config.js      # Tailwind config
├── vite.config.js          # Vite config
└── src/
    ├── main.jsx            # React entry point
    ├── App.jsx             # Top-level app (tabs + header + health bar)
    ├── index.css           # Tailwind imports + theme
    └── components/
        ├── ChatPanel.jsx   # Chat completion form
        ├── ModelPanel.jsx  # Model list + import
        ├── MonitorPanel.jsx # SSD/thermal/throughput dashboard
        └── HealthBar.jsx   # Bottom health status bar
```

### Add a new component

1. Create `ui/src/components/MyComponent.jsx`
2. Import it in `App.jsx`
3. Add a tab button in the header

### Styling

The UI uses [Tailwind CSS](https://tailwindcss.com/). Dark theme by default.
All classes are utility-first (e.g. `bg-slate-800 text-slate-200 px-3 py-2 rounded`).

### API communication

Components talk to the API server directly via `fetch()`:

```javascript
const resp = await fetch(`${apiUrl}/v1/chat/completions`, {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
  },
  body: JSON.stringify({ model, messages: [...], ... }),
})
const body = await resp.json()
```

The `apiUrl` and `apiKey` come from the header inputs in `App.jsx`.

---

## IPC Command Reference

The Rust backend exposes these IPC commands via `DesktopApp::handle_ipc()`:

| Command | HTTP Call | Returns |
|---|---|---|
| `GetHealth` | `GET /health` | `{"status":"ok","version":"0.1.0"}` |
| `ListModels` | `GET /v1/models` | `{"object":"list","data":[...]}` |
| `GetModel { model_id }` | `GET /v1/models/:id` | `{"id":"...","object":"model",...}` |
| `GetConfig` | (local) | `AppConfig` JSON |
| `ImportModel { path }` | `POST /v1/models/import` | `{"model_id":"...","status":"accepted"}` |
| `StartServer` | (local) | `{"status":"started"}` |
| `StopServer` | (local) | `{"status":"stopped"}` |
| `GetMonitorData` | (local) | `{"ssd_temp_c":45.0,"tok_per_sec":0.0,...}` |
| `Chat { model, prompt, ... }` | `POST /v1/chat/completions` | `{"choices":[...],"usage":{...}}` |
| `Embed { text }` | `POST /v1/embeddings` | `{"data":[...],"model":"..."}` |
| `Search { query, top_k }` | `POST /v1/embeddings/search` | `{"results":[...]}` |

### Using IPC from Rust

```rust
use edgeforge_app::{DesktopApp, IpcCommand};

let app = DesktopApp::new();
let resp = app.handle_ipc(IpcCommand::GetHealth).await;
if resp.success {
    println!("Health: {}", resp.data);
} else {
    println!("Error: {:?}", resp.error);
}
```

---

## Configuration

### AppConfig (Rust)

| Field | Type | Default | Purpose |
|---|---|---|---|
| `api_url` | `String` | `http://127.0.0.1:11434` | API server URL |
| `theme` | `String` | `dark` | UI theme |
| `window_width` | `u32` | `1200` | Window width (Tauri mode) |
| `window_height` | `u32` | `800` | Window height (Tauri mode) |
| `auto_start_api` | `bool` | `true` | Auto-start the API server |
| `api_key` | `Option<String>` | `None` | API key for auth |
| `timeout_secs` | `f64` | `30.0` | IPC HTTP timeout |

### tauri.conf.json

The Tauri config (`crates/edgeforge-app/src-tauri/tauri.conf.json`) controls:
- Window size + title
- Bundle identifier (`com.edgeforge.desktop`)
- Frontend dist path (`../ui/dist`)
- Dev server URL (`http://localhost:5173`)
- Build commands (auto-runs `npm run build` before bundling)

---

## Building a Distributable Bundle

### Linux

```bash
cd crates/edgeforge-app/ui
npm install
npm run tauri build
# Output: src-tauri/target/release/bundle/deb/edgeforge_0.1.0_amd64.deb
#         src-tauri/target/release/bundle/appimage/edgeforge_0.1.0_amd64.AppImage
```

### macOS

```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/dmg/EdgeForge_0.1.0_aarch64.dmg
#         src-tauri/target/release/bundle/macos/EdgeForge.app
```

### Windows

```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/msi/EdgeForge_0.1.0_x64_en-US.msi
#         src-tauri/target/release/bundle/nsis/EdgeForge_0.1.0_x64-setup.exe
```

---

## Troubleshooting

### "run() returns NotImplemented"

This was a **Phase 19 bug**. It is fixed: `run()` now succeeds in headless
mode (returns `Ok(())`). Update to the latest build:

```bash
cargo build --release -p edgeforge-app
```

### "No window opens" (headless mode)

This is expected in headless mode (the default build). To get a window:

```bash
cargo build --release -p edgeforge-app --features tauri-shell
```

Or use the Tauri CLI:

```bash
cd crates/edgeforge-app/ui
npm install
npm run tauri dev
```

### "error: failed to find tauri"

The `tauri-shell` feature is not enabled. Either:
- Build with `--features tauri-shell`
- Or use headless mode (default)

### "webkit2gtk-4.1 not found" (Linux)

Install the webkit dependencies:

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel
```

### "npm: command not found"

Install Node.js (18+):

```bash
# macOS
brew install node

# Ubuntu
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install nodejs
```

### The React UI can't connect to the API server

1. Check the API server is running: `curl http://127.0.0.1:11434/health`
2. Check the API URL in the UI header matches
3. Check the API key (if auth is enabled)

### How do I add a new tab to the UI?

1. Create a component in `ui/src/components/MyTab.jsx`
2. Add a tab button in `App.jsx`'s `<nav>`
3. Add a conditional render in `<main>`

### How do I add a new IPC command?

1. Add a variant to `IpcCommand` in `src/lib.rs`
2. Add a match arm in `IpcDispatchState::dispatch`
3. Implement the HTTP call in a new method on `IpcDispatchState`
4. Add a test in the `#[cfg(test)]` module
