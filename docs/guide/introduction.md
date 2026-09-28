---
title: ARM64 & Raspberry Pi 5 Docker Metrics Exporter
description: docker-exporter — ARM64 & Raspberry Pi 5 Docker metrics for Prometheus. A ~7 MiB Rust exporter with cAdvisor-compatible metric names and no privileged mode.
---

# docker-exporter for ARM64 & Raspberry Pi 5

**docker-exporter** is a [Prometheus](https://prometheus.io/) exporter for Docker container metrics, written in Rust and built for ARM64 homelabs running cgroup v2.

It reads the Docker stats API directly, computes the memory working set the way `docker stats` does on both cgroup versions, and idles around **7 MiB of RAM**. Metric names are cAdvisor-compatible, so existing Prometheus scrape configs and most Grafana dashboards work without a rewrite.

## The problem it solves

cAdvisor monitors the whole host, which is a lot of machinery for a single-board computer. It runs privileged with five host mounts and keeps walking the cgroup tree on a timer, whether anyone scrapes it or not. On a Raspberry Pi 5 with seven containers it used 0.87 to 1.72% of a core and 22.5 to 35.6 MiB, depending on its housekeeping interval, against docker-exporter's 0.13% and 4.2 MiB, for the same numbers ([full footprint benchmark →](/why/benchmark)). On this project's busier hosts it averaged 9 to 17% CPU, though with a flag that made it heavier.

::: tip Memory reads zero on a Raspberry Pi?
That's the Pi's boot configuration, not a bug in any exporter. The memory cgroup is disabled at boot, so `docker stats`, cAdvisor and docker-exporter all read zero until you add `cgroup_enable=memory`. [How to fix it →](/why/cadvisor-arm64-zero-memory)
:::

## What you get

- **Memory working set** as `docker stats` computes it, on cgroup v1 *and* v2 (`usage − inactive_file` on v2, `usage − cache` on v1).
- Per-container **CPU, memory, network, and block I/O**, plus state, health, and lifecycle metrics.
- A **read-only** Docker socket and **non-root** execution (UID 65532, no privileged mode).
- **On-demand collection** — no background polling; each scrape fetches stats with a 5 s per-container timeout.
- **Glob-aware** container exclusion via `EXCLUDE_CONTAINERS`.
- `/health` and `/ready` endpoints (the image's `HEALTHCHECK` is already wired).

## When to use it

`docker-exporter` is purpose-built for one job: per-container Docker metrics. cAdvisor monitors much more (host, processes, OOM events, hardware counters).

- **Use docker-exporter** if you already run Prometheus + Grafana and want a small-footprint, unprivileged cAdvisor alternative on an SBC.
- **Keep cAdvisor** if you need host/process metrics and its footprint isn't a problem.

See the [full docker-exporter vs cAdvisor comparison →](/compare/cadvisor).

## Next steps

- [Installation](/guide/installation) — Docker run, Compose, socket permissions.
- [Configuration](/guide/configuration) — environment variables and exclusion globs.
- [Metrics reference](/guide/metrics) — every metric, label, and endpoint.
- [Prometheus & Grafana](/guide/prometheus-grafana) — scrape config, alerting, and dashboards.
- [Security](/guide/security) — the read-only-socket, non-root posture.
