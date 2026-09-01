# Stage 1: Build (static musl)
FROM rust:1.87-bookworm@sha256:251cec8da4689d180f124ef00024c2f83f79d9bf984e43c180a598119e326b84 AS builder

# buildx provides TARGETARCH per native-runner build (amd64 / arm64).
ARG TARGETARCH

WORKDIR /build

# Map the Docker arch to its Rust musl triple. Build per-arch on native
# runners, so cross-compilation isn't needed — only the local musl target.
RUN case "${TARGETARCH}" in \
      amd64) echo "x86_64-unknown-linux-musl" > /tmp/musl-triple ;; \
      arm64) echo "aarch64-unknown-linux-musl" > /tmp/musl-triple ;; \
      *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && rustup target add "$(cat /tmp/musl-triple)" \
    && apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache dependency compilation against the musl
# triple (warming the host-glibc cache here would be wasted work).
RUN mkdir -p src && echo "fn main(){}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release --target "$(cat /tmp/musl-triple)" 2>/dev/null || true

# Copy real source code
COPY src/ src/

# Invalidate cached builds for real source
RUN touch src/main.rs src/lib.rs
RUN cargo build --release --target "$(cat /tmp/musl-triple)" --bin docker-exporter \
    && cp "target/$(cat /tmp/musl-triple)/release/docker-exporter" /tmp/docker-exporter

# Stage 2: Runtime (distroless static, non-root)
FROM gcr.io/distroless/static-debian12:nonroot@sha256:afa5c872c891853ca7fcf1f12c3edb23f7eeef36189728842dd51042ff57f7ab

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

# distroless:nonroot already ships ca-certificates and a non-root `nonroot`
# user (uid 65532) — no apt, no useradd needed. The static musl binary has
# no runtime library deps.
COPY --from=builder /tmp/docker-exporter /usr/local/bin/docker-exporter

USER nonroot:nonroot

EXPOSE 9713

# Native liveness probe — the binary TCP-connects to its own port on
# loopback and exits 0/1. Replaces the previous `wget` HEALTHCHECK so the
# image needs no shell or extra tooling. Absolute path: distroless has no PATH.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/usr/local/bin/docker-exporter", "--health"]

ENTRYPOINT ["/usr/local/bin/docker-exporter"]
