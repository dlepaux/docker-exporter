![CI](https://github.com/dlepaux/docker-exporter/actions/workflows/ci.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Image size](https://ghcr-badge.egpl.dev/dlepaux/docker-exporter/size)

# docker-exporter

A tiny **Prometheus exporter for Docker container metrics**, written in Rust for ARM64 homelabs running cgroup v2 — and the fix for cAdvisor's **zero-memory bug on Raspberry Pi 5**.

📖 **Full documentation: [docker-exporter.tech](https://docker-exporter.tech)**

cAdvisor reports zero for `container_memory_working_set_bytes` on Raspberry Pi 5 (ARM64 + cgroup v2) — a [known, upstream-unfixed bug](https://github.com/google/cadvisor/issues/3469). `docker-exporter` reads the Docker stats API directly, computes the working set correctly on both cgroup versions, talks to the socket **read-only**, runs **non-root**, and idles around **7 MiB of RAM**. Metric names are cAdvisor-compatible, so existing Grafana dashboards work unchanged.

## Quick start

```bash
docker run -d \
  --name docker-exporter \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 \
  --restart unless-stopped \
  ghcr.io/dlepaux/docker-exporter:latest
```

Then scrape `http://localhost:9713/metrics`. Image is published for `linux/amd64` and `linux/arm64`.

## Why docker-exporter

- **Correct memory working set** on cgroup v1 **and** v2 (`usage − inactive_file` on v2, `usage − cache` on v1) — the number cAdvisor zeroes on ARM64.
- **~9 MB image, ~7 MiB RAM, < 1% CPU** — a single static binary on distroless.
- **Read-only** Docker socket, **non-root** (UID 65532), no privileged mode.
- **On-demand** collection — no background loop; 5 s per-container timeout.
- Per-container CPU, memory, network, block I/O, state, health, and lifecycle metrics.
- Glob-aware container exclusion via `EXCLUDE_CONTAINERS`.

## Documentation

Everything lives at **[docker-exporter.tech](https://docker-exporter.tech)**:

- [Installation](https://docker-exporter.tech/guide/installation) · [Configuration](https://docker-exporter.tech/guide/configuration) · [Metrics reference](https://docker-exporter.tech/guide/metrics)
- [Prometheus & Grafana](https://docker-exporter.tech/guide/prometheus-grafana) · [Architecture](https://docker-exporter.tech/guide/architecture) · [Troubleshooting](https://docker-exporter.tech/guide/troubleshooting)
- [Why cAdvisor breaks on Raspberry Pi 5](https://docker-exporter.tech/why/cadvisor-arm64-zero-memory) · [docker-exporter vs cAdvisor](https://docker-exporter.tech/compare/cadvisor)

## Development

```bash
cargo build           # debug build
cargo test            # unit + integration tests (some need a local Docker daemon)
cargo clippy          # lints
cargo fmt             # format (CI enforces `cargo fmt --check`)
cargo build --release # release binary
```

Tests that need a Docker daemon skip themselves when the socket isn't reachable, so `cargo test` is safe to run anywhere.

## Security

Found a vulnerability? Follow the disclosure policy in [`SECURITY.md`](SECURITY.md) — please do **not** open a public issue.

## License

[MIT](license.md)
