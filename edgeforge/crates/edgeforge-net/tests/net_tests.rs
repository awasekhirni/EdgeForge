//! # Network Tests — edgeforge-net
//!
//! Integration tests for the full mesh pipeline:
//!   - LoopbackTransport: connect, open streams, heartbeat, close
//!   - QuinnTransport: delegates to loopback, full API surface works
//!   - mTLS: LocalCa::generate, sign_node_cert, verify_peer_cert (Ed25519)
//!   - Bandwidth: probe_bandwidth with loopback + Quinn transports
//!   - Pipeline: forward_activations with loopback + Quinn transports
//!   - Recovery: repage_from_ssd with real WeightPager + temp file
//!   - End-to-end: sign cert → connect → open stream → forward → repage

use edgeforge_core::{Bytes, ExpertId, LayerIdx, NodeId};
use edgeforge_net::*;
use edgeforge_storage::weight_pager::{WeightPager, WeightPagerConfig};
use edgeforge_storage::{BlockingIo, PlatformIo};
use std::sync::Arc;
use ulid::Ulid;

// ── Helpers ────────────────────────────────────────────────────────────────

fn peer_addr(port: u16) -> quic_transport::PeerAddr {
    quic_transport::PeerAddr(format!("127.0.0.1:{port}").parse().unwrap())
}

// ── LoopbackTransport tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_loopback_connect_succeeds() {
    let t = LoopbackTransport::new();
    let conn = t
        .connect(&peer_addr(9999))
        .await
        .expect("loopback connect should succeed");
    assert_eq!(conn.peer, peer_addr(9999));
    assert!(conn.metadata.established_at_ms > 0);
}

#[tokio::test]
async fn test_loopback_full_pipeline() {
    let t = LoopbackTransport::new();

    // 1. Connect
    let conn = t.connect(&peer_addr(9001)).await.unwrap();
    assert!(conn.metadata.established_at_ms > 0);

    // 2. Open weight stream
    let ws = t.open_weight_stream(&conn).await.unwrap();
    assert!(ws.stream_id > 0);

    // 3. Open activation stream
    let as_ = t.open_activation_stream(&conn).await.unwrap();
    assert!(as_.stream_id > ws.stream_id, "stream IDs should be monotonic");

    // 4. Heartbeat
    let hb = t.heartbeat(&conn).await.unwrap();
    assert!(hb.interval_ms > 0);
    assert!(hb.last_beat_ms > 0);

    // 5. Close
    t.close(&conn).await.unwrap();

    // 6. Stream opens should fail after close.
    assert!(t.open_weight_stream(&conn).await.is_err());
}

#[tokio::test]
async fn test_loopback_multiple_peers_independent() {
    let t = LoopbackTransport::new();
    let conn1 = t.connect(&peer_addr(9001)).await.unwrap();
    let conn2 = t.connect(&peer_addr(9002)).await.unwrap();
    assert_ne!(conn1.peer_id, conn2.peer_id);

    // Closing conn1 should not affect conn2.
    t.close(&conn1).await.unwrap();
    assert!(t.open_weight_stream(&conn2).await.is_ok());
    assert!(t.open_weight_stream(&conn1).await.is_err());
}

// ── QuinnTransport tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_quinn_connect_succeeds() {
    // Previously: asserted is_err(). Now: must succeed.
    let t = QuinnTransport::new();
    let conn = t
        .connect(&peer_addr(9999))
        .await
        .expect("QuinnTransport::connect should succeed");
    assert_eq!(conn.peer, peer_addr(9999));
}

#[tokio::test]
async fn test_quinn_full_pipeline() {
    let t = QuinnTransport::new();
    let conn = t.connect(&peer_addr(9001)).await.unwrap();
    let _ws = t.open_weight_stream(&conn).await.unwrap();
    let _as_ = t.open_activation_stream(&conn).await.unwrap();
    let _hb = t.heartbeat(&conn).await.unwrap();
    t.close(&conn).await.unwrap();
    assert!(t.open_weight_stream(&conn).await.is_err());
}

#[tokio::test]
async fn test_quinn_with_custom_config() {
    let config = TransportConfig {
        max_idle_timeout_ms: 60_000,
        max_bi_streams: 128,
        keep_alive_interval_ms: 10_000,
        ..Default::default()
    };
    let t = QuinnTransport::with_config(config);
    assert_eq!(t.config().max_idle_timeout_ms, 60_000);
    assert_eq!(t.config().max_bi_streams, 128);
    assert_eq!(t.config().keep_alive_interval_ms, 10_000);
    // Should still work.
    let conn = t.connect(&peer_addr(9001)).await.unwrap();
    assert!(t.open_weight_stream(&conn).await.is_ok());
}

// ── mTLS tests ─────────────────────────────────────────────────────────────

#[test]
fn test_local_ca_generate_succeeds() {
    let ca = LocalCa::generate().expect("LocalCa::generate should succeed");
    assert!(!ca.cert_pem.is_empty());
    assert!(!ca.key_pem.is_empty());
    assert_eq!(ca.fingerprint.len(), 64); // SHA-256 hex
}

#[test]
fn test_mtls_sign_and_verify_roundtrip() {
    let ca = LocalCa::generate().unwrap();
    let node_id = NodeId(Ulid::new());
    let cert = ca.sign_node_cert(node_id).unwrap();
    let verified = verify_peer_cert(&cert, &ca).unwrap();
    assert_eq!(verified, node_id);
}

#[test]
fn test_mtls_verify_fails_with_wrong_ca() {
    let ca1 = LocalCa::generate().unwrap();
    let ca2 = LocalCa::generate().unwrap();
    let node_id = NodeId(Ulid::new());
    let cert = ca1.sign_node_cert(node_id).unwrap();
    assert!(verify_peer_cert(&cert, &ca2).is_err());
}

#[test]
fn test_mtls_verify_fails_with_tampered_signature() {
    let ca = LocalCa::generate().unwrap();
    let node_id = NodeId(Ulid::new());
    let mut cert = ca.sign_node_cert(node_id).unwrap();
    cert.signature_bytes[0] ^= 0xFF;
    assert!(verify_peer_cert(&cert, &ca).is_err());
}

#[test]
fn test_mtls_each_ca_unique() {
    let ca1 = LocalCa::generate().unwrap();
    let ca2 = LocalCa::generate().unwrap();
    assert_ne!(ca1.fingerprint, ca2.fingerprint);
}

// ── Bandwidth tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_probe_bandwidth_loopback_succeeds() {
    let m = probe_bandwidth_loopback(peer_addr(9999))
        .await
        .expect("probe_bandwidth_loopback should succeed");
    assert!(m.bytes_per_second > 0);
    assert!(m.rtt.as_micros() > 0);
}

#[tokio::test]
async fn test_probe_bandwidth_with_quinn_transport() {
    let t = QuinnTransport::new();
    let m = probe_bandwidth(&t, peer_addr(9999))
        .await
        .expect("probe_bandwidth with QuinnTransport should succeed");
    assert!(m.bytes_per_second > 0);
}

#[test]
fn test_bandwidth_threshold_check() {
    let sufficient = BandwidthMeasurement {
        peer: NodeId(Ulid::new()),
        bytes_per_second: 200_000_000,
        rtt: std::time::Duration::from_millis(5),
    };
    assert!(bandwidth::is_sufficient_for_remote_fetch(&sufficient));

    let insufficient = BandwidthMeasurement {
        peer: NodeId(Ulid::new()),
        bytes_per_second: 50_000_000,
        rtt: std::time::Duration::from_millis(20),
    };
    assert!(!bandwidth::is_sufficient_for_remote_fetch(&insufficient));
}

// ── Pipeline tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_forward_activations_loopback_succeeds() {
    let result = forward_activations_loopback(NodeId(Ulid::new()), &[1, 2, 3, 4]).await;
    assert!(result.is_ok(), "forward_activations_loopback should succeed");
}

#[tokio::test]
async fn test_forward_activations_with_quinn_transport() {
    let t = QuinnTransport::new();
    let result = forward_activations(&t, peer_addr(9999), &[1, 2, 3]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_forward_activations_large_payload() {
    let t = QuinnTransport::new();
    let payload = vec![0u8; 1_000_000];
    let result = forward_activations(&t, peer_addr(9999), &payload).await;
    assert!(result.is_ok());
}

#[test]
fn test_pipeline_plan_round_robin() {
    let n1 = NodeId(Ulid::new());
    let n2 = NodeId(Ulid::new());
    let n3 = NodeId(Ulid::new());
    let plan = PipelinePlan::round_robin(9, &[n1, n2, n3]);
    assert_eq!(plan.len(), 9);
    assert_eq!(plan.node_for_layer(LayerIdx(0)), Some(n1));
    assert_eq!(plan.node_for_layer(LayerIdx(1)), Some(n2));
    assert_eq!(plan.node_for_layer(LayerIdx(2)), Some(n3));
    assert_eq!(plan.node_for_layer(LayerIdx(3)), Some(n1)); // wraps
}

// ── Recovery tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_repage_from_ssd_simple_succeeds() {
    let weights = vec![0xABu8; 4096];
    let paged = repage_from_ssd_simple("layer_3_expert_5", &weights)
        .await
        .expect("repage_from_ssd_simple should succeed");
    assert_eq!(paged.len(), 4096);
    assert_eq!(paged[0], 0xAB);
}

#[tokio::test]
async fn test_repage_from_ssd_with_real_weight_pager() {
    use edgeforge_storage::weight_pager::{WeightPageId, WeightPager, WeightPagerConfig};

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let data: Vec<u8> = (0..(6 * 4096)).map(|i| (i % 256) as u8).collect();
    std::fs::write(tmp.path(), &data).unwrap();

    let io: Arc<dyn PlatformIo> = Arc::new(BlockingIo::new());
    let pager = WeightPager::new(
        WeightPagerConfig {
            page_size: Bytes(4096),
            ram_budget: Bytes(1024 * 1024),
            ssd_path: tmp.path().to_path_buf(),
        },
        io,
        edgeforge_core::ModelId::new(),
    );

    let paged = repage_from_ssd(&pager, LayerIdx(5), ExpertId(7))
        .await
        .unwrap();
    assert_eq!(paged.len(), 4096);
    assert_eq!(paged[0], (20480 % 256) as u8);
}

#[tokio::test]
async fn test_repage_from_ssd_simple_invalid_id() {
    let result = repage_from_ssd_simple("not_valid", &[0u8; 16]).await;
    assert!(result.is_err());
}

#[test]
fn test_heartbeat_lifecycle() {
    let mut hb = recovery::Heartbeat::new(recovery::HeartbeatConfig {
        miss_threshold: 3,
        ..Default::default()
    });
    assert!(hb.is_alive());
    assert!(!hb.on_missed()); // 1
    assert!(!hb.on_missed()); // 2
    assert!(hb.on_missed()); // 3 → dead
    assert!(!hb.is_alive());
    // Revive
    hb.on_received(1000);
    assert!(hb.is_alive());
    assert_eq!(hb.missed_count(), 0);
}

// ── End-to-end mesh pipeline test ──────────────────────────────────────────

#[tokio::test]
async fn e2e_mesh_pipeline() {
    // 1. Generate a local CA + sign a node cert (mTLS).
    let ca = LocalCa::generate().expect("CA generation should succeed");
    let node_id = NodeId(Ulid::new());
    let cert = ca
        .sign_node_cert(node_id)
        .expect("node cert signing should succeed");
    let verified = verify_peer_cert(&cert, &ca).expect("cert should verify");
    assert_eq!(verified, node_id);

    // 2. Connect to a peer via the mesh transport.
    let transport = QuinnTransport::new();
    let conn = transport
        .connect(&peer_addr(11435))
        .await
        .expect("connect should succeed");

    // 3. Probe bandwidth.
    let bw = probe_bandwidth(&transport, peer_addr(11435))
        .await
        .expect("bandwidth probe should succeed");
    assert!(bw.bytes_per_second > 0);

    // 4. Open weight + activation streams.
    let _ws = transport
        .open_weight_stream(&conn)
        .await
        .expect("weight stream should open");
    let _as_ = transport
        .open_activation_stream(&conn)
        .await
        .expect("activation stream should open");

    // 5. Forward activations.
    let activations = vec![0u8; 1024];
    forward_activations(&transport, peer_addr(11435), &activations)
        .await
        .expect("forward_activations should succeed");

    // 6. Re-page an expert from SSD (simulating peer loss recovery).
    let weights = vec![0xCDu8; 8192];
    let paged = repage_from_ssd_simple("layer_2_expert_4", &weights)
        .await
        .expect("repage should succeed");
    assert_eq!(paged, weights);

    // 7. Close the connection.
    transport
        .close(&conn)
        .await
        .expect("close should succeed");

    tracing::info!("E2E mesh pipeline completed successfully");
}

// ── Mesh event channel tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_mesh_event_channel() {
    let (mesh, mut rx) = Mesh::new();
    assert_eq!(mesh._events_tx.capacity(), 64);
    // Receiver should be empty initially.
    let _ = rx.try_recv();
}
