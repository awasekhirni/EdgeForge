# edgeforge-app

EdgeForge desktop application shell — Tauri 2 + React UI (ADR-006).

## Quick Start

### Headless mode (default, no window)

```bash
cargo build --release -p edgeforge-app
./target/release/edgeforge-app  # prints help, returns Ok
```

In headless mode, the app doesn't open a window. Run the API server + UI
separately:

```bash
# Terminal 1: API server
./target/release/edgeforge serve &

# Terminal 2: React UI (talks to the API server)
cd crates/edgeforge-app/ui
npm install
npm run dev
# Open http://localhost:5173
```

### Tauri mode (real native window)

Requires system webview libraries:
- **Linux**: `webkit2gtk-4.1-dev`, `libgtk-3-dev`, `libappindicator3-dev`
- **macOS**: Xcode + Command Line Tools
- **Windows**: WebView2 (preinstalled on Win10/11)

```bash
cd crates/edgeforge-app/ui
npm install
npm run tauri dev
```

Or build a bundle:

```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/{deb,app,dmg,msi,...}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Tauri Window (native webview)                              │
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

## IPC Commands

The Rust backend exposes these IPC commands (via `DesktopApp::handle_ipc`):

| Command | HTTP Call | Purpose |
|---|---|---|
| `GetHealth` | `GET /health` | Check server health |
| `ListModels` | `GET /v1/models` | List imported models |
| `GetModel` | `GET /v1/models/:id` | Get a single model |
| `GetConfig` | (local) | Get the app config |
| `ImportModel` | `POST /v1/models/import` | Import a model |
| `StartServer` | (local) | Mark API server as started |
| `StopServer` | (local) | Mark API server as stopped |
| `GetMonitorData` | (local) | Get SSD/thermal/throughput |
| `Chat` | `POST /v1/chat/completions` | Chat completion |
| `Embed` | `POST /v1/embeddings` | Generate embedding |
| `Search` | `POST /v1/embeddings/search` | RAG similarity search |

## UI Components

- `App.jsx` — top-level app with tab navigation + API URL/key inputs
- `ChatPanel.jsx` — chat completion form + response display
- `ModelPanel.jsx` — model list + import form
- `MonitorPanel.jsx` — SSD temp, throughput, health dashboard
- `HealthBar.jsx` — bottom bar showing server health (polls every 5s)

## Configuration

The app reads its config from `AppConfig` (in Rust):

| Field | Default | Purpose |
|---|---|---|
| `api_url` | `http://127.0.0.1:11434` | API server URL |
| `theme` | `dark` | UI theme |
| `window_width` | `1200` | Window width (Tauri mode) |
| `window_height` | `800` | Window height (Tauri mode) |
| `auto_start_api` | `true` | Auto-start the API server |
| `api_key` | `None` | API key for auth |
| `timeout_secs` | `30.0` | IPC HTTP timeout |

The React UI also has its own API URL + API key inputs in the header
(overrides the Rust config at runtime).

## License

Apache-2.0
