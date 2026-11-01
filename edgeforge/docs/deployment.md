# Deployment Guide

This guide covers deploying EdgeForge in production environments.

## Table of Contents

1. [Docker Deployment](#docker-deployment)
2. [Bare Metal Deployment](#bare-metal-deployment)
3. [Nix Deployment](#nix-deployment)
4. [Configuration](#configuration)
5. [Monitoring](#monitoring)
6. [Security Hardening](#security-hardening)
7. [Backup & Recovery](#backup--recovery)

---

## Docker Deployment

### Quick Start

```bash
# Clone the repo
git clone https://github.com/your-org/edgeforge.git
cd edgeforge

# Build and run
docker-compose up -d

# Verify
curl http://127.0.0.1:11434/health
```

### Configuration via Environment Variables

All config values can be overridden via environment variables prefixed with `EDGEFORGE_`:

```yaml
# docker-compose.yml
services:
  edgeforge:
    environment:
      - EDGEFORGE_DATA=/data
      - RUST_LOG=info
      - EDGEFORGE_API__LISTEN_ADDR=0.0.0.0:11434  # accept external connections
      - EDGEFORGE_API__REQUIRE_API_KEY=true
      - EDGEFORGE_STORAGE__DEFAULT_PRECISION=fp16
```

### Volume Mounts

| Mount | Purpose |
|---|---|
| `/data` | SQLite database, logs, cache |
| `/models` | Model weight files (GGUF, SafeTensors) |

### Health Check

The container includes a built-in health check:

```bash
docker inspect --format='{{.State.Health.Status}}' edgeforge
```

### Logs

```bash
# Follow logs
docker-compose logs -f edgeforge

# View structured JSON logs
docker-compose logs edgeforge | jq .
```

---

## Bare Metal Deployment

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
```

### 2. Build

```bash
git clone --recurse-submodules https://github.com/your-org/edgeforge.git
cd edgeforge
cargo build --release --bin edgeforge
```

### 3. Create a Systemd Service (Linux)

```ini
# /etc/systemd/system/edgeforge.service
[Unit]
Description=EdgeForge API Server
After=network.target

[Service]
Type=simple
User=edgeforge
Group=edgeforge
WorkingDirectory=/opt/edgeforge
ExecStart=/opt/edgeforge/edgeforge serve
Restart=on-failure
RestartSec=5
Environment=EDGEFORGE_DATA=/var/lib/edgeforge
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable edgeforge
sudo systemctl start edgeforge
sudo systemctl status edgeforge
```

### 4. Create a LaunchAgent (macOS)

```xml
<!-- ~/Library/LaunchAgents/dev.edgeforge.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>dev.edgeforge</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/edgeforge</string>
        <string>serve</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>EDGEFORGE_DATA</key>
        <string>/usr/local/var/lib/edgeforge</string>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

```bash
launchctl load ~/Library/LaunchAgents/dev.edgeforge.plist
```

---

## Nix Deployment

### Hermetic Build (ADR-028)

```bash
# Enter a dev shell with all tooling pinned
nix develop

# Build the workspace
nix build

# The binary is at ./result/bin/edgeforge
./result/bin/edgeforge serve
```

### Reproducibility

The `flake.nix` pins:
- Rust 1.82.0 via `rust-overlay`
- All system libraries (cmake, llvm, openssl, sqlite) by hash via `nixpkgs`
- Build inputs via `naersk` with `singleStep = true`

Release artifacts are bit-for-bit reproducible from the same `git SHA + flake.lock`.

---

## Configuration

### Default Config File

Location: `~/.config/edgeforge/config.toml` (or `EDGEFORGE_CONFIG` env var).

### Key Settings

```toml
[api]
listen_addr = "127.0.0.1:11434"    # Use 0.0.0.0:11434 for external access
require_api_key = false             # Set to true for production
rate_limit_per_minute = 600

[storage]
min_ssd_gib = 2048                  # 2 TB minimum (ADR-002)
default_precision = "fp16"          # ADR-003
use_direct_io = true                # Bypass OS page cache
thermal_poll_interval_secs = 5      # F-MON-04

[network]
mode = "air_gapped"                 # Default; change to "lan_open" for mesh
```

---

## Monitoring

### Health Endpoint

```bash
curl http://127.0.0.1:11434/health
# {"status":"ok","version":"0.1.0"}
```

### OpenTelemetry (Opt-In — ADR-012)

```toml
[observability]
otlp_enabled = true
otlp_endpoint = "http://localhost:4317"
log_level = "info"
```

**Safety (R-14):** Span attributes never include prompt content. The `FORBIDDEN_ATTRIBUTE_SUBSTRINGS` denylist is enforced.

### SSD Thermal Monitoring (F-MON-04)

EdgeForge polls SSD temperature every 5 seconds:
- **70°C** — Prefetch depth reduced (throttle)
- **80°C** — Inference paused (critical)

### Carbon-Aware Routing (ADR-026)

```toml
[carbon]
enabled = true
latency_slo_ms = 500
intensity_source = "static"  # or "electricity_maps" or "watttime"
```

---

## Security Hardening

### 1. Require API Key

```toml
[api]
require_api_key = true
```

### 2. Enable Air-Gapped Mode (Default)

```toml
[network]
mode = "air_gapped"
telemetry = false
```

### 3. Enable Prompt Security (ADRs 020–023)

```toml
[promptsec]
enabled = true
fast_path_for_toolless = true
```

### 4. Enable Hybrid PQC (ADR-027 — Beta Channel)

```bash
edgeforge config set updates.channel beta
edgeforge config set crypto.kem hybrid_x25519_ml_kem_768
edgeforge config set crypto.sig hybrid_ed25519_ml_dsa_65
```

### 5. Review Audit Logs

```sql
-- Prompt security audit
SELECT * FROM prompt_guardrail_events WHERE verdict = 'reject' ORDER BY created_at DESC LIMIT 20;

-- Verification events (Tier 2/3)
SELECT * FROM verification_events ORDER BY created_at DESC LIMIT 20;

-- Autonomic actions
SELECT * FROM autonomic_actions ORDER BY created_at DESC LIMIT 20;
```

---

## Backup & Recovery

### What to Back Up

1. **SQLite database** — `~/.local/share/edgeforge/edgeforge.db`
2. **Model manifests** — your curated list + license acceptances
3. **Config file** — `~/.config/edgeforge/config.toml`

### What NOT to Back Up

- **Model weights** — re-downloadable from Hugging Face
- **KV cache** — ephemeral, regenerated on demand
- **Expert cache** — ephemeral, paged from SSD

### Recovery

```bash
# Restore the database
cp backup/edgeforge.db ~/.local/share/edgeforge/edgeforge.db

# Re-import models
edgeforge model import --path /path/to/model.gguf

# Verify
edgeforge model list
```
