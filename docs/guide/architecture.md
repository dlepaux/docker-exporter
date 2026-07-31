---
title: Architecture
description: "docker-exporter's architecture: no background loop, per-scrape stats, and cgroup v2-aware working set math — the calculation cAdvisor gets wrong on ARM64."
---

# Architecture

docker-exporter has **no background loop**. Everything happens on the request.

## Per-scrape collection

On each `GET /metrics`, the exporter:

1. Lists all containers, running **and** stopped, so `container_state` covers every state.
2. For every container, concurrently fetches **stats** (`bollard::stats(stream=false)`) and an **inspect** (health + restart policy + exit code), each with a **5 s timeout**. Containers are processed at a **bounded concurrency of 64**, not all at once.
3. Encodes the result as Prometheus text.

Stats are skipped for containers that are neither `running` nor `paused`. Docker closes the stats stream empty for them, so the call could only fail, and their CPU/memory series are zero either way — skipping changes no output, it just avoids a doomed request.

Failed or timed-out calls are logged and counted: **inspect** failures increment `docker_exporter_inspect_failures_total`, **stats** failures increment `docker_exporter_stats_failures_total`. The container is still emitted either way — a stats failure zeroes its CPU/memory/network/block I/O, an inspect failure sets `health="none"`, `restart_policy="unknown"` and drops the container's `container_exit_code` series entirely (rather than reporting a fabricated `0`, which would read as a successful job) — so one bad container never fails the whole scrape. When failures are widespread the per-container log lines are sampled (10 per kind per scrape) and followed by one aggregate line; the counters carry the exact totals.

### Why concurrency is bounded

Each in-flight container may hold **two** socket connections at once (stats + inspect), and hyper opens a fresh file descriptor per connection — a pooled HTTP/1.1 connection serves one request at a time, and the pool caps only *idle* connections, never concurrent ones.

An unbounded fan-out over `N` containers therefore attempts `~2N` simultaneous `connect()` calls. Past the process `RLIMIT_NOFILE` (soft **1024** under Docker's default) every further connect fails with `EMFILE`, which hyper reports only as the opaque `client error (Connect)` — the errno lives in a `source` that `Display` never prints. On a host with a few thousand containers this turns every scrape into thousands of warnings while the exporter still reports `docker_exporter_up 1`.

Bounding the fan-out caps peak descriptors at `2 × 64` regardless of `N`. The cost is negligible: 2080 containers inspect in about **0.4 s**.

With no polling, the process sits idle between scrapes, so scrape duration tracks Docker daemon latency — the `/containers/{id}/stats` and `/containers/{id}/json` calls — rather than the exporter's own encoding work.

## Working set computation

The working set is computed at exposition time from the Docker stats payload:

- **cgroup v2:** `max(0, usage − inactive_file)`
- **cgroup v1:** `max(0, usage − cache)`

This is the calculation cAdvisor gets wrong on ARM64 + cgroup v2, where it reports zero — [cAdvisor #3469](https://github.com/google/cadvisor/issues/3469), closed *"not planned"* on 2025-12-09. [Why cAdvisor reports zero memory on ARM64 →](/why/cadvisor-arm64-zero-memory)

## State across scrapes

The Prometheus output is rebuilt from scratch each scrape as `MetricFamily` protos (no `Registry`), so nothing stale carries over and counters emit as absolute values. The only cross-scrape state is the two failure counters, `docker_exporter_inspect_failures_total` and `docker_exporter_stats_failures_total` — `AtomicU64`s in the shared `AppState` that accumulate since start, by design. Everything else is per-request, keeping memory flat and behavior predictable under concurrent scrapes.

## Footprint

- **Memory:** ~7–10 MiB idle, ~10–20 MiB scraping ~30 containers.
- **CPU:** near-zero at rest; a scrape's work scales with container count and daemon latency, but steady-state stays **under 1%** on a Raspberry Pi 5.
- **Image:** ~9 MB — a static musl binary on `distroless/static`, non-root, identical on `linux/amd64` and `linux/arm64`.

See the [footprint benchmark →](/why/benchmark) for the methodology and numbers versus cAdvisor, or the [full docker-exporter vs cAdvisor comparison →](/compare/cadvisor) for a feature-by-feature breakdown.
