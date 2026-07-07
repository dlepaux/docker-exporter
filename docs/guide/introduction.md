---
title: ARM64 & Raspberry Pi 5 Docker Metrics Exporter
description: docker-exporter — ARM64 & Raspberry Pi 5 Docker metrics for Prometheus. A Rust exporter that fixes cAdvisor's zero-memory bug on cgroup v2, ~7 MiB RAM.
---

# docker-exporter for ARM64 & Raspberry Pi 5

**docker-exporter** is a [Prometheus](https://prometheus.io/) exporter for Docker container metrics, written in Rust and built for ARM64 homelabs running cgroup v2.

It reads the Docker stats API directly, computes the memory working set correctly on both cgroup versions, and idles around **7 MiB of RAM**. Metric names are cAdvisor-compatible, so existing Prometheus scrape configs and Grafana dashboards work without a rewrite.

## The problem it solves

On a Raspberry Pi 5 (ARM64 + cgroup v2), cAdvisor returns **zero** for `container_memory_working_set_bytes`, so memory dashboards silently break — part of cAdvisor's broader ARM-side pain ([#2523](https://github.com/google/cadvisor/issues/2523)).

This is upstream-acknowledged and unfixed: [cAdvisor #3469](https://github.com/google/cadvisor/issues/3469) was closed *"not planned"* on 2025-12-09, and it persists **even after** you enable memory cgroups on the Pi. [Read the full cAdvisor ARM64 zero-memory bug story →](/why/cadvisor-arm64-zero-memory)

cAdvisor's footprint is also large for a single-board computer: 9–17% sustained CPU, ~80–150 MiB RAM per host, privileged mode, and six mounted host paths — because it monitors the whole host, not just Docker ([full footprint benchmark →](/why/benchmark)).

## What you get

- **Correct memory working set** on cgroup v1 *and* v2 (`usage − inactive_file` on v2, `usage − cache` on v1).
- Per-container **CPU, memory, network, and block I/O**, plus state, health, and lifecycle metrics.
- A **read-only** Docker socket and **non-root** execution (UID 65532, no privileged mode).
- **On-demand collection** — no background polling; each scrape fetches stats with a 5 s per-container timeout.
- **Glob-aware** container exclusion via `EXCLUDE_CONTAINERS`.
- `/health` and `/ready` endpoints (the image's `HEALTHCHECK` is already wired).

## When to use it

`docker-exporter` is purpose-built for one job: per-container Docker metrics. cAdvisor monitors much more (host, processes, OOM events, hardware counters).

- **Use docker-exporter** if you already run Prometheus + Grafana and either hit the ARM64/cgroup v2 memory bug or want a tiny-footprint cAdvisor alternative on an SBC.
- **Keep cAdvisor** if you need host/process metrics and its footprint isn't a problem.

See the [full docker-exporter vs cAdvisor comparison →](/compare/cadvisor).

## Next steps

- [Installation](/guide/installation) — Docker run, Compose, socket permissions.
- [Configuration](/guide/configuration) — environment variables and exclusion globs.
- [Metrics reference](/guide/metrics) — every metric, label, and endpoint.
- [Prometheus & Grafana](/guide/prometheus-grafana) — scrape config, alerting, and dashboards.
- [Security](/guide/security) — the read-only-socket, non-root posture.
