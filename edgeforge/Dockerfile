# EdgeForge Dockerfile — headless API server + Python SDK wheel.
# Multi-stage build: builder + Python wheel builder + minimal runtime image.
#
# Build:
#   docker build -t edgeforge:latest .
#
# Run:
#   docker run -p 11434:11434 -v edgeforge-data:/data edgeforge:latest
#
# The container runs the API server only (no Tauri UI). Mount a volume
# at /data for persistent model storage. The Python SDK wheel is built
# and placed in /wheels/ for extraction.

# ── Stage 1: Rust builder ────────────────────────────────────────────
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
COPY pyproject.toml ./
COPY python/ ./python/

# Build the CLI binary in release mode.
RUN cargo build --release --bin edgeforge

# ── Stage 2: Python wheel builder ────────────────────────────────────
FROM rust:1.82-slim AS wheel-builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Install maturin for wheel building.
RUN pip3 install maturin

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY resources/ ./resources/
COPY pyproject.toml ./
COPY python/ ./python/

# Build the Python SDK wheel.
RUN maturin build --release -m crates/edgeforge-sdk-python/Cargo.toml

# ── Stage 3: Runtime ─────────────────────────────────────────────────
FROM debian:bookworm-slim

# Install runtime dependencies.
RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create the data directory.
RUN mkdir -p /data /models /wheels

# Copy the binary from the builder.
COPY --from=builder /build/target/release/edgeforge /usr/local/bin/edgeforge

# Copy the Python SDK wheel from the wheel builder.
COPY --from=wheel-builder /build/target/wheels/ /wheels/

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
