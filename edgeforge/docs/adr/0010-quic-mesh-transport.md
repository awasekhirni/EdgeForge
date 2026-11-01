# ADR-010: QUIC Mesh Transport

**Status:** Approved
**Date:** 2026-10-31

## Decision

Mesh transport uses QUIC via `quinn` 0.11 with mTLS via `rustls`. All streams are multiplexed over a single QUIC connection per peer.

## Rationale

QUIC provides superior NAT traversal (F-NET-02 relay) compared to TCP. Built-in TLS ensures encryption. Stream multiplexing avoids head-of-line blocking between weight fetches, activations, and heartbeats.

## Upgrade

ADR-027 upgrades the mTLS handshake to hybrid post-quantum cryptography (X25519 + ML-KEM-768, Ed25519 + ML-DSA-65).
