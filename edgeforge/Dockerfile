# EdgeForge Dockerfile — headless API server (no desktop UI).
# Multi-stage build: builder + minimal runtime image.
#
# Build:
#   docker build -t edgeforge:latest .
#
# Run:
#   docker run -p 11434:11434 -v edgeforge-data:/data edgeforge:latest
#
# The container runs the API server only (no Tauri UI). Mount a volume
# at /data for persistent model storage.

# ── Builder stage ─────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

# Install build dependencies.
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    libclang-dev \
    llvm \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy the workspace.
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY resources/ ./resources/
COPY scripts/ ./scripts/

# Build the CLI binary in release mode.
RUN cargo build --release --bin edgeforge

# ── Runtime stage ─────────────────────────────────────────────────────
FROM debian:bookworm-slim

# Install runtime dependencies.
RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create the data directory.
RUN mkdir -p /data /models

# Copy the binary from the builder.
COPY --from=builder /build/target/release/edgeforge /usr/local/bin/edgeforge

# Set the data directory.
ENV EDGEFORGE_DATA=/data
ENV RUST_LOG=info

# Expose the API port.
EXPOSE 11434

# Health check.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://127.0.0.1:11434/health || exit 1

# Run the API server.
ENTRYPOINT ["edgeforge"]
CMD ["serve"]
