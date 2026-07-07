---
title: Architecture
description: "docker-exporter's architecture: no background loop, per-scrape stats, and cgroup v2-aware working set math — the calculation cAdvisor gets wrong on ARM64."
---

# Architecture

docker-exporter has **no background loop**. Everything happens on the request.

## Per-scrape collection

On each `GET /metrics`, the exporter:

1. Lists all containers, running **and** stopped, so `container_state` covers every state.
2. For every container, concurrently fetches **stats** (`bollard::stats(stream=false)`) and an **inspect** (health + restart policy), each with a **5 s timeout**; all containers run in parallel.
3. Encodes the result as Prometheus text.

Failed or timed-out calls are logged; only **inspect** failures increment `docker_exporter_inspect_failures_total`. The container is still emitted either way — a stats failure zeroes its CPU/memory/network/block I/O, an inspect failure sets `health="none"` and `restart_policy="unknown"` — so one bad container never fails the whole scrape.

With no polling, the process sits idle between scrapes, so scrape duration tracks Docker daemon latency — the concurrent `/containers/{id}/stats` and `/containers/{id}/json` calls per container — rather than the exporter's own encoding work.

## Working set computation

The working set is computed at exposition time from the Docker stats payload:

- **cgroup v2:** `max(0, usage − inactive_file)`
- **cgroup v1:** `max(0, usage − cache)`

This is the calculation cAdvisor gets wrong on ARM64 + cgroup v2, where it reports zero — [cAdvisor #3469](https://github.com/google/cadvisor/issues/3469), closed *"not planned"* on 2025-12-09. [Why cAdvisor reports zero memory on ARM64 →](/why/cadvisor-arm64-zero-memory)

## State across scrapes

The Prometheus output is rebuilt from scratch each scrape as `MetricFamily` protos (no `Registry`), so nothing stale carries over and counters emit as absolute values. The one piece of cross-scrape state is `docker_exporter_inspect_failures_total` — an `AtomicU64` in the shared `AppState` that accumulates since start, by design. Everything else is per-request, keeping memory flat and behavior predictable under concurrent scrapes.

## Footprint

- **Memory:** ~7–10 MiB idle, ~10–20 MiB scraping ~30 containers.
- **CPU:** near-zero at rest; a scrape's work scales with container count and daemon latency, but steady-state stays **under 1%** on a Raspberry Pi 5.
- **Image:** ~9 MB — a static musl binary on `distroless/static`, non-root, identical on `linux/amd64` and `linux/arm64`.

See the [footprint benchmark →](/why/benchmark) for the methodology and numbers versus cAdvisor, or the [full docker-exporter vs cAdvisor comparison →](/compare/cadvisor) for a feature-by-feature breakdown.
