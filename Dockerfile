# Multi-stage build for minimal ConceptKernel container (~20-30MB)
# Builder stage - compile Rust binary
FROM rust:1.83-bookworm AS builder

WORKDIR /build

# Install build dependencies (including C++ compiler for oxrocksdb-sys)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    clang && \
    rm -rf /var/lib/apt/lists/*

# Copy Cargo files first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY core-rs ./core-rs/

# Build release binary with optimizations and strip symbols
RUN cargo build --release --bin ckr && \
    strip /build/target/release/ckr

# Runtime stage - Google Distroless (minimal glibc-based image ~20MB)
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy binary from builder
COPY --from=builder /build/target/release/ckr /usr/local/bin/ckr

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Set working directory
WORKDIR /ckp

# Distroless runs as nonroot user by default (UID 65532)
USER nonroot:nonroot

# Health check (distroless doesn't have shell, so can't do traditional healthcheck)
# GitHub Actions and orchestrators will need to use HTTP/TCP checks instead

# Default command - show help
ENTRYPOINT ["/usr/local/bin/ckr"]
CMD ["--help"]
