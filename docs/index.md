---
layout: home
title: docker-exporter — Docker metrics for Raspberry Pi 5, ARM64 & cgroup v2
titleTemplate: false
description: Correct Docker memory metrics on ARM64 & cgroup v2 (Raspberry Pi 5) — a ~7 MiB Rust Prometheus exporter and drop-in cAdvisor alternative for homelabs.
hero:
  name: docker-exporter
  text: Correct Docker metrics on ARM64 & cgroup v2
  tagline: A tiny Rust Prometheus exporter for Docker containers — and the fix for cAdvisor's zero-memory bug on Raspberry Pi 5.
  image:
    src: /logo.svg
    alt: docker-exporter
  actions:
    - theme: brand
      text: Get started
      link: /guide/introduction
    - theme: alt
      text: Why cAdvisor breaks on Pi
      link: /why/cadvisor-arm64-zero-memory
    - theme: alt
      text: View on GitHub
      link: https://github.com/dlepaux/docker-exporter

features:
  - icon: 🎯
    title: Correct working set
    details: "usage − inactive_file on cgroup v2, usage − cache on v1. The exact number cAdvisor reports as zero on ARM64 + cgroup v2."
  - icon: 🪶
    title: ~7 MiB RAM, ~9 MB image
    details: A single static Rust binary on distroless/static. Under 1% CPU at steady state — sized for a Raspberry Pi, not a datacenter.
  - icon: 🔒
    title: Read-only & non-root
    details: Mounts the Docker socket read-only and runs as UID 65532. No privileged mode, no cgroup/proc/sys bind mounts.
  - icon: 🔁
    title: Drop-in for cAdvisor
    details: cAdvisor-compatible metric names — plugs into an existing Prometheus scrape and Grafana dashboards with no rewrites.
  - icon: ⚡
    title: No background loop
    details: Stats are fetched per scrape with a 5 s per-container timeout. Every scrape rebuilds all metric families from scratch — no cached or stale metric state between requests.
  - icon: 🧩
    title: Glob container exclusion
    details: "EXCLUDE_CONTAINERS with glob patterns (cache-*, *-sidecar). A malformed pattern fails loudly at startup, never silently."
---

<div class="de-stats">
  <div class="de-stat"><div class="de-stat-num">~7 MiB</div><div class="de-stat-label">RAM at idle</div></div>
  <div class="de-stat"><div class="de-stat-num">~9 MB</div><div class="de-stat-label">image (musl + distroless)</div></div>
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

On a Raspberry Pi 5 (ARM64 + cgroup v2), cAdvisor reports **zero** for `container_memory_working_set_bytes` and `container_memory_rss` — memory dashboards silently lie. It's an upstream-acknowledged bug ([cAdvisor #3469](https://github.com/google/cadvisor/issues/3469), closed *"not planned"*) that persists **even after** you enable memory cgroups on the Pi.

`docker-exporter` reads the Docker stats API directly and computes the working set correctly on both cgroup versions. It talks to the socket read-only and runs [non-root](/guide/security). [Read the full story →](/why/cadvisor-arm64-zero-memory)

## docker-exporter vs cAdvisor

| Dimension | docker-exporter | cAdvisor |
| --- | --- | --- |
| Image size | **~9 MB** | ~250 MB |
| RAM (idle, ~10 containers) | **~7–10 MiB** | ~80–150 MiB |
| cgroup v2 working set on ARM64 | **Correct** | Reports zero — [known issue](https://github.com/google/cadvisor/issues/3469) |
| Privileged container | **No** (socket read-only) | Yes |
| Scope | Docker containers | Containers + host + processes + hardware |

Numbers are docker-exporter's own reported measurements — see the [reproducible benchmark methodology →](/why/benchmark).

Already running cAdvisor and happy with it? Keep it. Hitting the Pi 5 memory bug, or want a smaller footprint on an SBC? [See the full comparison →](/compare/cadvisor)
