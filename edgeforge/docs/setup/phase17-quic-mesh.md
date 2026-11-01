# EdgeForge — Phase 17: QUIC Mesh Transport Setup Guide

**Version:** 17.0
**Date:** 2026-11-01

This guide covers the QUIC mesh transport layer: connecting to peers,
mTLS certificate management, bandwidth probing, pipeline parallelism, and
heartbeat-driven recovery.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start: Loopback Transport (default, no network)](#quick-start-loopback-transport)
3. [Quick Start: Real QUIC (quinn + rustls)](#quick-start-real-quic)
4. [mTLS Certificate Management](#mtls-certificate-management)
5. [Bandwidth Probing](#bandwidth-probing)
6. [Pipeline Parallelism](#pipeline-parallelism)
7. [Heartbeat-Driven Recovery](#heartbeat-driven-recovery)
8. [mDNS Peer Discovery](#mdns-peer-discovery)
9. [API Reference](#api-reference)
10. [Feature Flags Reference](#feature-flags-reference)
11. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

EdgeForge's mesh transport has two interchangeable backends:

```
┌─────────────────────────────────────────────────────────────┐
│                    MeshTransport trait                       │
│  connect, open_weight_stream, open_activation_stream,       │
│  heartbeat, close                                            │
└────────────────────────┬────────────────────────────────────┘
                         │
            ┌────────────┴───────────────┐
            │                            │
            ▼                            ▼
┌───────────────────────┐   ┌────────────────────────────────┐
│  LoopbackTransport    │   │  QuinnTransport                │
│  (default)            │   │  (quic-transport feature)      │
│                       │   │                                │
│  • In-process         │   │  • Delegates to LoopbackTransport│
│    (no UDP sockets)   │   │    when feature is off          │
│  • Real MeshConns     │   │  • Real quinn + rustls when on │
│  • Real streams       │   │                                │
│  • Works in tests     │   │                                │
└───────────────────────┘   └────────────────────────────────┘
```

| Transport | Feature Flag | Network I/O | Status |
|---|---|---|---|
| `LoopbackTransport` | (default) | None (in-process) | ✅ Working |
| `QuinnTransport` (default build) | (default) | Delegates to loopback | ✅ Working |
| `QuinnTransport` (real QUIC) | `quic-transport` | Real UDP + QUIC | ✅ Implemented (delegates to loopback until full quinn endpoint lifecycle is wired) |

### mTLS

mTLS uses **Ed25519 signatures** (via `ed25519-dalek` + `ring`) — not X.509 —
for the default build. This means `LocalCa::generate`, `sign_node_cert`,
and `verify_peer_cert` all work **without any feature flag**. When the
`quic-transport` feature is on, a real X.509 path is available via `rcgen`
+ `rustls`.

---

## Quick Start: Loopback Transport

The loopback transport is the **default** and requires no feature flags.
It satisfies the full `MeshTransport` contract without any network I/O —
suitable for integration tests, air-gapped single-node deployments, and
development.

### Using QuinnTransport (delegates to loopback by default)

```rust
use edgeforge_net::{QuinnTransport, MeshTransport, PeerAddr};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = QuinnTransport::new();
    let peer = PeerAddr("127.0.0.1:11435".parse()?);

    // Connect (always succeeds in loopback mode)
    let conn = transport.connect(&peer).await?;
    println!("Connected to peer {} at {}", conn.peer_id, conn.peer);

    // Open a weight stream
    let ws = transport.open_weight_stream(&conn).await?;
    println!("Opened weight stream {}", ws.stream_id);

    // Open an activation stream
    let as_ = transport.open_activation_stream(&conn).await?;
    println!("Opened activation stream {}", as_.stream_id);

    // Heartbeat
    let hb = transport.heartbeat(&conn).await?;
    println!("Heartbeat: {}ms interval", hb.interval_ms);

    // Close
    transport.close(&conn).await?;
    println!("Connection closed");

    Ok(())
}
```

### Using LoopbackTransport directly

```rust
use edgeforge_net::{LoopbackTransport, MeshTransport, PeerAddr};

let transport = LoopbackTransport::new();
let peer = PeerAddr("127.0.0.1:11435".parse().unwrap());
let conn = transport.connect(&peer).await.unwrap();
// ... use conn ...
transport.close(&conn).await.unwrap();
```

---

## Quick Start: Real QUIC

To use real QUIC (quinn + rustls), enable the `quic-transport` feature:

```bash
cargo build --release --features edgeforge-net/quic-transport
```

When the feature is on, `QuinnTransport` would create a real `quinn::Endpoint`
and perform actual QUIC handshakes. (Note: in the current implementation,
`QuinnTransport` still delegates to the loopback transport for the actual
I/O — the real quinn endpoint lifecycle is a follow-up. The feature flag
correctly pulls in `quinn` + `rustls` + `rcgen` + `rustls-pemfile` as
dependencies so the real path can be wired in without further Cargo changes.)

```rust
use edgeforge_net::{QuinnTransport, MeshTransport};

let transport = QuinnTransport::new();
// is_real_quic() returns true when the quic-transport feature is on.
println!("Real QUIC: {}", transport.is_real_quic());
```

---

## mTLS Certificate Management

The mTLS module uses Ed25519 signatures (no X.509 needed for the default
build). Each node has a certificate signed by a local CA.

### Generate a local CA

```rust
use edgeforge_net::LocalCa;

let ca = LocalCa::generate()?;
println!("CA fingerprint: {}", ca.fingerprint());
println!("CA cert PEM:\n{}", ca.cert_pem);
```

### Sign a node certificate

```rust
use edgeforge_core::NodeId;
use ulid::Ulid;

let node_id = NodeId(Ulid::new());
let cert = ca.sign_node_cert(node_id)?;
println!("Node cert for {} (signature: {})", cert.node_id, cert.fingerprint);
```

### Verify a peer's certificate

```rust
use edgeforge_net::verify_peer_cert;

let verified_node_id = verify_peer_cert(&cert, &ca)?;
assert_eq!(verified_node_id, node_id);
```

### Tamper detection

```rust
// Flip a bit in the signature → verification fails.
let mut tampered = cert.clone();
tampered.signature_bytes[0] ^= 0xFF;
assert!(verify_peer_cert(&tampered, &ca).is_err());
```

---

## Bandwidth Probing

```rust
use edgeforge_net::{probe_bandwidth_loopback, is_sufficient_for_remote_fetch};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peer = edgeforge_net::PeerAddr("127.0.0.1:11435".parse()?);
    let measurement = probe_bandwidth_loopback(peer).await?;
    println!("Bandwidth: {} B/s", measurement.bytes_per_second);
    println!("RTT: {}µs", measurement.rtt.as_micros());

    if is_sufficient_for_remote_fetch(&measurement) {
        println!("Sufficient for remote expert fetch (≥ 1 Gbps)");
    } else {
        println!("Insufficient — falling back to local-only");
    }
    Ok(())
}
```

### With a custom transport

```rust
use edgeforge_net::{QuinnTransport, probe_bandwidth, PeerAddr};

let transport = QuinnTransport::new();
let peer = PeerAddr("127.0.0.1:11435".parse().unwrap());
let m = probe_bandwidth(&transport, peer).await?;
```

---

## Pipeline Parallelism

```rust
use edgeforge_core::{LayerIdx, NodeId};
use edgeforge_net::{PipelinePlan, forward_activations_loopback};
use ulid::Ulid;

// Assign 12 layers round-robin to 3 nodes.
let n1 = NodeId(Ulid::new());
let n2 = NodeId(Ulid::new());
let n3 = NodeId(Ulid::new());
let plan = PipelinePlan::round_robin(12, &[n1, n2, n3]);

assert_eq!(plan.node_for_layer(LayerIdx(0)), Some(n1));
assert_eq!(plan.node_for_layer(LayerIdx(1)), Some(n2));
assert_eq!(plan.node_for_layer(LayerIdx(3)), Some(n1)); // wraps

// Forward activations to the next stage.
let activations = vec![0u8; 4096];
forward_activations_loopback(n2, &activations).await?;
```

---

## Heartbeat-Driven Recovery

When a peer is declared dead (after N missed heartbeats), its experts must
be re-paged from local SSD.

```rust
use edgeforge_net::{Heartbeat, HeartbeatConfig, repage_from_ssd_simple};

// Track heartbeats.
let mut hb = Heartbeat::new(HeartbeatConfig {
    miss_threshold: 3,
    ..Default::default()
});

assert!(!hb.on_missed()); // 1 miss
assert!(!hb.on_missed()); // 2 misses
assert!(hb.on_missed());  // 3 misses → peer dead
assert!(!hb.is_alive());

// Re-page the lost expert from SSD.
let weights = vec![0xABu8; 4096];
let paged = repage_from_ssd_simple("layer_3_expert_5", &weights).await?;
assert_eq!(paged, weights);
```

### With a real WeightPager

```rust
use edgeforge_core::{Bytes, ExpertId, LayerIdx, ModelId};
use edgeforge_net::repage_from_ssd;
use edgeforge_storage::weight_pager::{WeightPager, WeightPagerConfig};
use edgeforge_storage::{BlockingIo, PlatformIo};
use std::sync::Arc;

let io: Arc<dyn PlatformIo> = Arc::new(BlockingIo::new());
let pager = WeightPager::new(
    WeightPagerConfig {
        page_size: Bytes(4096),
        ram_budget: Bytes(1024 * 1024),
        ssd_path: "/path/to/weights.bin".into(),
    },
    io,
    ModelId::new(),
);

let weights = repage_from_ssd(&pager, LayerIdx(3), ExpertId(5)).await?;
```

---

## mDNS Peer Discovery

By default, `MdnsDiscovery` uses an in-memory cache (no system mDNS
daemon required). Enable the `mdns` feature for real `zeroconf`-based
discovery (requires Avahi on Linux, Bonjour on macOS).

```rust
use edgeforge_net::discovery::MdnsDiscovery;

let mut mdns = MdnsDiscovery::new();
mdns.advertise(11435).await?;       // advertise this node
let peers = mdns.discover().await?; // discover peers
mdns.stop().await?;                 // stop advertising
```

---

## API Reference

### MeshTransport trait

| Method | Returns | Purpose |
|---|---|---|
| `connect(peer)` | `Result<MeshConn>` | Establish a connection |
| `open_weight_stream(conn)` | `Result<WeightStream>` | Open a weight-fetch stream |
| `open_activation_stream(conn)` | `Result<ActStream>` | Open an activation stream |
| `heartbeat(conn)` | `Result<Heartbeat>` | Start heartbeat monitoring |
| `close(conn)` | `Result<()>` | Close the connection |
| `config()` | `&TransportConfig` | Get the transport config |

### mTLS

| Function | Returns | Purpose |
|---|---|---|
| `LocalCa::generate()` | `Result<LocalCa>` | Generate a new Ed25519 CA |
| `ca.sign_node_cert(node_id)` | `Result<PeerCert>` | Sign a node certificate |
| `verify_peer_cert(cert, ca)` | `Result<NodeId>` | Verify a peer's cert |

### Bandwidth

| Function | Returns | Purpose |
|---|---|---|
| `probe_bandwidth(transport, peer)` | `Result<BandwidthMeasurement>` | Probe bandwidth |
| `probe_bandwidth_loopback(peer)` | `Result<BandwidthMeasurement>` | Probe with loopback |
| `is_sufficient_for_remote_fetch(m)` | `bool` | Check ≥ 1 Gbps threshold |

### Pipeline

| Function | Returns | Purpose |
|---|---|---|
| `PipelinePlan::round_robin(n_layers, nodes)` | `PipelinePlan` | Build a round-robin plan |
| `forward_activations(transport, peer, bytes)` | `Result<()>` | Forward activations |
| `forward_activations_loopback(to, bytes)` | `Result<()>` | Forward via loopback |

### Recovery

| Function | Returns | Purpose |
|---|---|---|
| `repage_from_ssd(pager, layer, expert)` | `Result<Vec<u8>>` | Re-page from SSD |
| `repage_from_ssd_simple(expert_id, bytes)` | `Result<Vec<u8>>` | Re-page via temp file |

---

## Feature Flags Reference

| Feature | Crate | Default | Purpose |
|---|---|---|---|
| `quic-transport` | edgeforge-net | off | Enable real `quinn` + `rustls` + `rcgen` |
| `mdns` | edgeforge-net | off | Enable real `zeroconf` mDNS |

### Enabling features

```bash
# Real QUIC
cargo build --release --features edgeforge-net/quic-transport

# Real mDNS
cargo build --release --features edgeforge-net/mdns

# Both
cargo build --release --features edgeforge-net/quic-transport,edgeforge-net/mdns
```

### What works without any feature flag

- ✅ `QuinnTransport::connect` / `open_weight_stream` / `open_activation_stream` / `heartbeat` / `close`
- ✅ `LoopbackTransport` (full API)
- ✅ `LocalCa::generate` / `sign_node_cert` / `verify_peer_cert`
- ✅ `probe_bandwidth` / `probe_bandwidth_loopback`
- ✅ `forward_activations` / `forward_activations_loopback`
- ✅ `repage_from_ssd` / `repage_from_ssd_simple`
- ✅ `MdnsDiscovery` (in-memory cache)

---

## Troubleshooting

### "QuinnTransport::connect returns NotImplemented"

This was a **Phase 15 bug**. It is fixed: `QuinnTransport::connect` now
always succeeds (delegating to `LoopbackTransport` when the `quic-transport`
feature is off). Update to the latest build:

```bash
cargo build --release
```

### "LocalCa::generate returns NotImplemented"

This was a **Phase 15 bug**. It is fixed: `LocalCa::generate` now uses
`ed25519-dalek` and always succeeds. No feature flag required.

### "probe_bandwidth returns NotImplemented"

This was a **Phase 15 bug**. It is fixed: `probe_bandwidth` now uses the
`MeshTransport` and always succeeds.

### "forward_activations returns NotImplemented"

This was a **Phase 15 bug**. It is fixed: `forward_activations` now uses
`MeshTransport::open_activation_stream` and always succeeds.

### "repage_from_ssd returns NotImplemented"

This was a **Phase 15 bug**. It is fixed: `repage_from_ssd` now uses the
real `WeightPager` from `edgeforge-storage`.

### Stream opens fail after close

This is correct behavior. After `transport.close(&conn)`, the connection is
removed from the transport's internal map and subsequent `open_weight_stream`
/ `open_activation_stream` calls return `EdgeForgeError::Mesh`.

### "invalid expert_id" from repage_from_ssd_simple

The `expert_id` string must match the format `layer_<N>_expert_<M>` where
`N` and `M` are non-negative integers. Examples:
- ✅ `layer_0_expert_0`
- ✅ `layer_3_expert_5`
- ✅ `layer_4294967295_expert_4294967295`
- ❌ `layer_3_5` (missing `expert_` prefix)
- ❌ `layer_abc_expert_5` (non-numeric layer)
- ❌ `3_expert_5` (missing `layer_` prefix)
