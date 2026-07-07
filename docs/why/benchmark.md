---
title: "ARM64/Raspberry Pi 5 benchmark: docker-exporter vs cAdvisor footprint"
description: "ARM64/Raspberry Pi 5 benchmark: docker-exporter idles ~7-10 MiB RAM, <1% CPU vs cAdvisor's ~80-150 MiB, 9-17% CPU. Reproducible methodology inside."
---

# ARM64/Raspberry Pi 5 footprint benchmark: docker-exporter vs cAdvisor

**Short answer:** on a small ARM64 host, docker-exporter idles around **~7–10 MiB RAM** with a **~9 MB image** and **under 1% CPU**, versus cAdvisor's **~80–150 MiB RAM**, **~250 MB image**, and a **sustained 9–17% CPU** floor. The gap is structural, not a tuning artifact — cAdvisor is built to introspect the whole host, docker-exporter only reads Docker.

::: info About these numbers
The figures below combine the project's reported measurements with cAdvisor's documented behavior and upstream issue reports. Absolute numbers vary with host, container count, and daemon load — **measure on your own hardware** before quoting them as hard limits. The methodology section makes that reproducible. And footprint is only half the story: on ARM64, cAdvisor also under-reports container memory as zero — [a two-layer bug that persists even after you enable memory cgroups](/why/cadvisor-arm64-zero-memory).
:::

## Results

| Dimension | docker-exporter | cAdvisor |
| --- | --- | --- |
| Image size | **~9 MB** (musl + distroless) | ~250 MB |
| RAM, idle (~10 containers) | **~7–10 MiB** | ~80–150 MiB |
| RAM, scraping ~30 containers | **~10–20 MiB** | no fixed figure — grows with host topology |
| CPU, steady state | **< 1%** | 9–17% sustained (project-reported) |
| Privileged mode | **No** | Yes |
| Bind mounts | **1** (socket, read-only) | 6 (rootfs, /var/run, /sys, /var/lib/docker, /dev/disk, /dev/kmsg) |

## Why the CPU gap is structural

The common cAdvisor workaround — `--docker_only=true` and `--housekeeping_interval=10s` — reduces CPU but doesn't eliminate it. That's a mitigation, not a fix: even with `docker_only`, cAdvisor still walks the full host/kernel/hardware topology by design — its own README pitches running "a single cAdvisor to monitor the whole machine." Users have long reported this CPU cost, as in issues like [#2523](https://github.com/google/cadvisor/issues/2523) and [#1897](https://github.com/google/cadvisor/issues/1897).

docker-exporter has no such floor because it does no host introspection. It has **no background loop** — work happens only during a scrape, and each scrape calls the Docker stats API. Between scrapes, CPU is idle. [Architecture →](/guide/architecture)

## Methodology (reproduce it)

On a Raspberry Pi 5 (or any host), with a representative container set running:

```bash
# 1. Image size
docker image ls ghcr.io/dlepaux/docker-exporter:latest --format '{{.Size}}'

# 2. Idle RAM (no scrape in flight) — watch the exporter's own container
docker stats --no-stream docker-exporter

# 3. RAM under load — hit /metrics in a tight loop for ~30s, then re-check
while true; do curl -s localhost:9713/metrics >/dev/null; done &
sleep 30; docker stats --no-stream docker-exporter; kill %1

# 4. Scrape duration (daemon-bound, not exporter-bound)
curl -s localhost:9713/metrics | grep docker_exporter_scrape_duration_seconds
```

Repeat steps 2–4 against a cAdvisor container to compare. Report container count and host arch alongside the numbers — they're the variables that move the results most.

## What this means in practice

If you're running a Prometheus + Grafana stack on a Pi or SBC, cAdvisor's footprint is a real tax on a small host — and on ARM64 it also [reports memory as zero](/why/cadvisor-arm64-zero-memory). docker-exporter trades away host/process scope for a fraction of the footprint and correct container memory. If you *need* host/process metrics, cAdvisor is the right tool — see the [full docker-exporter vs cAdvisor comparison](/compare/cadvisor).
