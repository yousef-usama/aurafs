# Multi-stage Dockerfile for aurafs-cloud
# Designed for production Kubernetes deployments on AWS EKS

# Stage 1: Build binary with cargo
FROM rust:bookworm AS builder

WORKDIR /usr/src/aurafs

# Pre-copy manifests for dependency layer caching
COPY Cargo.toml Cargo.lock ./
COPY aurafs-core/Cargo.toml aurafs-core/
COPY aurafs-daemon/Cargo.toml aurafs-daemon/
COPY aurafs-cloud/Cargo.toml aurafs-cloud/

# Copy full source tree
COPY aurafs-core aurafs-core
COPY aurafs-daemon aurafs-daemon
COPY aurafs-cloud aurafs-cloud

# Compile production release binary for aurafs-cloud
RUN cargo build --release --bin aurafs-cloud

# Stage 2: Minimal, secure runtime container
FROM debian:bookworm-slim

# Install CA certificates for TLS AWS API communication
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create unprivileged service user and group (Container Security Best Practice)
RUN groupadd -g 10001 aurafs && \
    useradd -u 10001 -g aurafs -s /bin/false aurafs

WORKDIR /app

# Copy compiled binary from builder
COPY --from=builder /usr/src/aurafs/target/release/aurafs-cloud /usr/local/bin/aurafs-cloud

# Set permissions and switch to unprivileged user
RUN chown -R aurafs:aurafs /app
USER 10001:10001

# Expose HTTP health and metrics port
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/aurafs-cloud"]
