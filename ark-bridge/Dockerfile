# ─────────────────────────────────────────────────────────────────────────────
# ark-bridge Dockerfile
#   Runs the TypeScript JSON-RPC bridge that exposes ARK client-side validation
#   to the Rust swap engine. Uses a distroless Node runtime for minimal attack
#   surface.
# ─────────────────────────────────────────────────────────────────────────────

# ── Stage 1: Dependencies ────────────────────────────────────────────────────
FROM node:20-bookworm-slim AS deps
WORKDIR /app

# Copy only the manifest files first for optimal layer caching.
COPY ark-bridge/package.json ark-bridge/package-lock.json ./

# Install production + dev deps (we need ts-node at runtime for now;
# a future optimisation would pre-compile to dist/).
RUN npm ci --ignore-scripts

# ── Stage 2: Builder (TypeScript compile) ───────────────────────────────────
FROM node:20-bookworm-slim AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY ark-bridge/ .

# Compile TypeScript → dist/
RUN npm run build

# ── Stage 3: Runtime ────────────────────────────────────────────────────────
FROM node:20-bookworm-slim AS runtime

# OCI image labels
LABEL org.opencontainers.image.title="satspath-ark-bridge" \
      org.opencontainers.image.description="SatsPath ARK bridge — TypeScript JSON-RPC validator for Ark client-side verification" \
      org.opencontainers.image.vendor="SatsPath" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.source="https://github.com/satspath/satspath"

# Security hardening
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN groupadd --system --gid 10002 bridge \
    && useradd  --system --uid 10002 --gid bridge \
                --no-create-home --shell /sbin/nologin bridge

WORKDIR /app

# Copy compiled JS and only production deps
COPY --from=builder /app/dist ./dist
COPY --from=deps    /app/node_modules ./node_modules
COPY ark-bridge/package.json ./

USER bridge

# JSON-RPC server port (internal; not exposed to the internet)
EXPOSE 3000

ENV NODE_ENV=production \
    ARK_BRIDGE_PORT=3000

CMD ["node", "dist/index.js"]
