![CI](https://github.com/dlepaux/docker-exporter/actions/workflows/ci.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Image size](https://ghcr-badge.egpl.dev/dlepaux/docker-exporter/size)

# docker-exporter

A tiny **Prometheus exporter for Docker container metrics**, written in Rust for ARM64 homelabs running cgroup v2: about **7 MiB of RAM**, well under **1% CPU**, and no privileged mode.

📖 **Full documentation: [docker-exporter.tech](https://docker-exporter.tech)**

`docker-exporter` reads the Docker stats API when Prometheus scrapes, instead of walking the host's cgroups on a timer. It computes the working set the way `docker stats` does on both cgroup versions, talks to the socket **read-only** and runs **non-root**. Measured side by side with cAdvisor on two ARM64 hosts, it used 7 to 30 times less CPU and 5 to 84 times less memory for the same container numbers, and the gap grows with the host's load ([benchmark](https://docker-exporter.tech/why/benchmark)). Metric names are cAdvisor-compatible, so most existing Grafana dashboards work unchanged.

Memory reading zero on a Raspberry Pi? That's the Pi's boot configuration, and it affects every tool, cAdvisor and this one alike: [one kernel flag fixes it](https://docker-exporter.tech/why/cadvisor-arm64-zero-memory).

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

- **Memory working set** as `docker stats` computes it, on cgroup v1 **and** v2 (`usage − inactive_file` on v2, `usage − cache` on v1).
- **~10 MB image (3.4 MB to pull), ~7 MiB RAM, < 1% CPU** — a single static binary on distroless.
- **Read-only** Docker socket, **non-root** (UID 65532), no privileged mode.
- **On-demand** collection — no background loop; 5 s per-container timeout.
- Per-container CPU, memory, network, block I/O, state, health, and lifecycle metrics.
- Glob-aware container exclusion via `EXCLUDE_CONTAINERS`.

## Documentation

Everything lives at **[docker-exporter.tech](https://docker-exporter.tech)**:

- [Installation](https://docker-exporter.tech/guide/installation) · [Configuration](https://docker-exporter.tech/guide/configuration) · [Metrics reference](https://docker-exporter.tech/guide/metrics)
- [Prometheus & Grafana](https://docker-exporter.tech/guide/prometheus-grafana) · [Architecture](https://docker-exporter.tech/guide/architecture) · [Troubleshooting](https://docker-exporter.tech/guide/troubleshooting)
- [Why memory reads zero on a Raspberry Pi 5](https://docker-exporter.tech/why/cadvisor-arm64-zero-memory) · [Footprint benchmark](https://docker-exporter.tech/why/benchmark) · [docker-exporter vs cAdvisor](https://docker-exporter.tech/compare/cadvisor)

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
