# ─────────────────────────────────────────────────────────────────────────────
# Stage 0 — Chef Planner
#   Computes the exact dependency recipe from Cargo manifests.
#   Rebuilds ONLY when Cargo.toml / Cargo.lock change, not when src/ changes.
#
# NOTE: fully-qualified image names are required for Podman compatibility.
# ─────────────────────────────────────────────────────────────────────────────
FROM docker.io/lukemathwalker/cargo-chef:latest-rust-1.80-bookworm AS chef
WORKDIR /build

# ─────────────────────────────────────────────────────────────────────────────
# Stage 1 — Planner
# ─────────────────────────────────────────────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2 — Cacher
#   Fetches and pre-compiles ALL dependencies from the recipe.
#   This layer is invalidated only when Cargo dependencies change.
# ─────────────────────────────────────────────────────────────────────────────
FROM chef AS cacher
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# ─────────────────────────────────────────────────────────────────────────────
# Stage 3 — Builder
#   Compiles only the application source. Re-runs only on src/ changes.
# ─────────────────────────────────────────────────────────────────────────────
FROM chef AS builder
# Restore pre-built dependency artifacts
COPY --from=cacher /build/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
# Copy full workspace source
COPY . .
# Build the release binary — this is the `cargo build --release` the user wants
RUN cargo build --release --bin satspath

# ─────────────────────────────────────────────────────────────────────────────
# Stage 4 — Runtime
#   Minimal Debian Bookworm Slim: glibc for OpenSSL, zero build tools.
#   Runs as a non-root user with a read-only root filesystem.
# ─────────────────────────────────────────────────────────────────────────────
FROM docker.io/debian:bookworm-slim AS runtime

# OCI image labels — provenance and registry cataloging.
LABEL org.opencontainers.image.title="satspath-cli" \
      org.opencontainers.image.description="SatsPath — universal Bitcoin payment resolver and router" \
      org.opencontainers.image.vendor="SatsPath" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.source="https://github.com/satspath/satspath" \
      org.opencontainers.image.documentation="https://github.com/satspath/satspath/blob/main/README.md"

# Security: install only the minimum required CA certificates (needed for HTTPS).
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Security: create a dedicated non-root user for running the binary.
RUN groupadd --system --gid 10001 satspath \
    && useradd  --system --uid 10001 --gid satspath \
                --no-create-home --shell /sbin/nologin satspath

# Copy the compiled binary (the only artifact we need from the builder stage).
COPY --from=builder /build/target/release/satspath /usr/local/bin/satspath
RUN chmod 755 /usr/local/bin/satspath

# Create a writable data directory for .satspath/ state (mounted as a volume).
RUN mkdir -p /data && chown satspath:satspath /data

# Drop to non-root
USER satspath
WORKDIR /data

# Default environment (overridable at runtime via -e or docker-compose env file).
ENV SATSPATH_DATA_DIR=/data

# satspath is a CLI tool; the default command shows help.
ENTRYPOINT ["/usr/local/bin/satspath"]
CMD ["--help"]
