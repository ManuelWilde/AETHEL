# ═══════════════════════════════════════════════════
# AETHEL — Multi-stage Docker build
# ═══════════════════════════════════════════════════

# Stage 1: Build
FROM rust:1.77-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first (for layer caching)
COPY Cargo.toml ./
COPY contracts/Cargo.toml contracts/Cargo.toml
COPY engine/Cargo.toml engine/Cargo.toml
COPY storage/Cargo.toml storage/Cargo.toml
COPY cli/Cargo.toml cli/Cargo.toml
COPY api/Cargo.toml api/Cargo.toml
COPY integration-tests/Cargo.toml integration-tests/Cargo.toml

# Create dummy sources for dependency caching
RUN mkdir -p contracts/src engine/src storage/src cli/src api/src integration-tests/tests && \
    echo "pub fn dummy() {}" > contracts/src/lib.rs && \
    echo "pub fn dummy() {}" > engine/src/lib.rs && \
    echo "pub fn dummy() {}" > storage/src/lib.rs && \
    echo "fn main() {}" > cli/src/main.rs && \
    echo "fn main() {}" > api/src/main.rs && \
    echo "" > integration-tests/tests/e2e.rs

# Build dependencies only (cached layer)
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
COPY contracts/src contracts/src
COPY engine/src engine/src
COPY storage/src storage/src
COPY cli/src cli/src
COPY api/src api/src
COPY integration-tests/tests integration-tests/tests

# Build everything
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binaries
COPY --from=builder /app/target/release/aethel /usr/local/bin/aethel
COPY --from=builder /app/target/release/aethel-server /usr/local/bin/aethel-server

# Create data directory
RUN mkdir -p /app/data

# Environment
ENV AETHEL_DB=/app/data/aethel.db
ENV AETHEL_PORT=3000
ENV RUST_LOG=info

# Initialize database on first run
RUN aethel --db /app/data/aethel.db init 2>/dev/null || true

EXPOSE 3000

# Default: run API server
CMD ["aethel-server"]
