---
layout: home
title: docker-exporter — Docker metrics for Raspberry Pi 5, ARM64 & cgroup v2
titleTemplate: false
description: Docker container metrics for Prometheus on ARM64 & cgroup v2 (Raspberry Pi 5) — a ~7 MiB Rust exporter and drop-in cAdvisor alternative for homelabs.
hero:
  name: docker-exporter
  text: Lightweight Docker metrics for ARM64 & cgroup v2
  tagline: A tiny Rust Prometheus exporter for Docker containers — about 7 MiB of RAM, a tenth of cAdvisor's CPU on a Raspberry Pi 5, no privileged mode.
  image:
    src: /logo.svg
    alt: docker-exporter
  actions:
    - theme: brand
      text: Get started
      link: /guide/introduction
    - theme: alt
      text: Memory reads zero on a Pi?
      link: /why/cadvisor-arm64-zero-memory
    - theme: alt
      text: View on GitHub
      link: https://github.com/dlepaux/docker-exporter

features:
  - icon: 🎯
    title: The working set docker stats shows
    details: "usage − inactive_file on cgroup v2, usage − cache on v1: the same number `docker stats` reports, under cAdvisor's metric name."
  - icon: 🪶
    title: ~7 MiB RAM, ~10 MB image
    details: A single static Rust binary on distroless/static. Under 1% CPU at steady state — sized for a Raspberry Pi, not a datacenter.
  - icon: 🔒
    title: Read-only & non-root
    details: Mounts the Docker socket read-only and runs as UID 65532. No privileged mode, no cgroup/proc/sys bind mounts.
  - icon: 🔁
    title: Drop-in for cAdvisor
    details: cAdvisor-compatible metric names — plugs into an existing Prometheus scrape, and most Grafana dashboards need no rewrite.
  - icon: ⚡
    title: No background loop
    details: Stats are fetched per scrape with a 5 s per-container timeout. Every scrape rebuilds all metric families from scratch — no cached or stale metric state between requests.
  - icon: 🧩
    title: Glob container exclusion
    details: "EXCLUDE_CONTAINERS with glob patterns (cache-*, *-sidecar). A malformed pattern fails loudly at startup, never silently."
---

<div class="de-stats">
  <div class="de-stat"><div class="de-stat-num">~7 MiB</div><div class="de-stat-label">RAM at idle</div></div>
  <div class="de-stat"><div class="de-stat-num">~10 MB</div><div class="de-stat-label">image (musl + distroless)</div></div>
  <div class="de-stat"><div class="de-stat-num">&lt;1%</div><div class="de-stat-label">CPU at steady state</div></div>
  <div class="de-stat"><div class="de-stat-num">2</div><div class="de-stat-label">arches: amd64 · arm64</div></div>
</div>

## Quick start

```bash
docker run -d \
  --name docker-exporter \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 \
  --restart unless-stopped \
  ghcr.io/dlepaux/docker-exporter:latest
```

Then scrape `http://localhost:9713/metrics`. Full walkthrough in the [installation guide](/guide/installation).

## Why it exists

cAdvisor watches the whole host. It runs privileged with five host mounts and keeps walking the cgroup tree on a timer, whether anyone scrapes it or not. On a Raspberry Pi 5 it used ten times docker-exporter's CPU to report the same four containers ([benchmark →](/why/benchmark)).

`docker-exporter` does one job. It reads the Docker stats API when Prometheus scrapes, computes the working set the way `docker stats` does on both cgroup versions, talks to the socket read-only and runs [non-root](/guide/security).

Memory reading zero on a Pi? That's the boot configuration, and it hits every tool alike, cAdvisor and docker-exporter included. [One flag fixes it →](/why/cadvisor-arm64-zero-memory)

## docker-exporter vs cAdvisor

| Dimension | docker-exporter | cAdvisor |
| --- | --- | --- |
| Image size (arm64) | **10.3 MB** (3.4 MB to pull) | 63.7 MB (27.7 MB to pull) |
| RAM (same Pi 5, 4 containers) | **7.4 MiB** | 19–29 MiB |
| CPU (same Pi 5, share of one core) | **0.10%** | 1.07% |
| Privileged container | **No** (socket read-only) | Yes |
| Scope | Docker containers | Containers + host + processes + hardware |

Measured side by side on a Raspberry Pi 5 on 2026-09-28, docker-exporter 1.6.0 against cAdvisor v0.60.6 — see the [benchmark and its method →](/why/benchmark).

Already running cAdvisor and happy with it? Keep it. Want a smaller footprint on an SBC? [See the full comparison →](/compare/cadvisor)
