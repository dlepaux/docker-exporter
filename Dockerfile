# Stage 1: Build
FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache dependency compilation
RUN mkdir -p src && echo "fn main(){}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release 2>/dev/null || true

# Copy real source code
COPY src/ src/

# Invalidate cached builds for real source
RUN touch src/main.rs src/lib.rs
RUN cargo build --release --bin docker-exporter

# Stage 2: Runtime
FROM debian:bookworm-slim

# Build metadata, baked into OCI labels by CI (defaults are placeholders for
# local builds where these args are not passed).
ARG VERSION=dev
ARG GIT_SHA=unknown
ARG BUILD_DATE=unknown

LABEL org.opencontainers.image.source=https://github.com/dlepaux/docker-exporter
LABEL org.opencontainers.image.licenses=MIT
LABEL org.opencontainers.image.title="docker-exporter"
LABEL org.opencontainers.image.description="Lightweight Prometheus exporter for Docker container metrics — built for ARM64 and cgroup v2"
LABEL org.opencontainers.image.version=$VERSION
LABEL org.opencontainers.image.revision=$GIT_SHA
LABEL org.opencontainers.image.created=$BUILD_DATE

# Upgrade base packages (security patches) before installing runtime deps.
RUN apt-get update \
    && apt-get upgrade -y \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system --gid 1001 exporter \
    && useradd --system --uid 1001 --gid exporter exporter

COPY --from=builder /build/target/release/docker-exporter /usr/local/bin/docker-exporter

USER exporter

EXPOSE 9713

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget -q --spider http://127.0.0.1:9713/health || exit 1

ENTRYPOINT ["docker-exporter"]
