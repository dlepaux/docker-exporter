---
title: docker-exporter vs cAdvisor — ARM64 / Raspberry Pi 5
description: docker-exporter vs cAdvisor on Raspberry Pi 5 / ARM64 — image size, RAM, CPU, privileged mode, and cgroup v2 memory correctness, compared.
head:
  - - script
    - type: application/ld+json
    - |-
      {"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"Is docker-exporter a drop-in replacement for cAdvisor?","acceptedAnswer":{"@type":"Answer","text":"For per-container Docker metrics, largely yes: docker-exporter uses cAdvisor-compatible metric names (container_cpu_usage_seconds_total, container_memory_working_set_bytes, etc.), so most Prometheus scrape configs and Grafana dashboards work unchanged. It does not replace cAdvisor's host, process, or hardware metrics."}},{"@type":"Question","name":"Why does cAdvisor report zero memory on Raspberry Pi 5?","acceptedAnswer":{"@type":"Answer","text":"Two independent problems stack. Raspberry Pi OS ships with memory cgroups disabled by default, and even after you enable them, cAdvisor still reports zero on ARM64 with cgroup v2 — upstream issue #3469, closed 'not planned'. docker-exporter computes the working set itself and reports correct memory."}}]}
---

# docker-exporter vs cAdvisor on ARM64 / Raspberry Pi 5

**Which should you run to get Docker container metrics into Prometheus on ARM64 / Raspberry Pi 5?** If you already run Prometheus + Grafana and want correct, low-footprint per-container metrics, use **docker-exporter** — a lightweight, ARM64-correct alternative to cAdvisor, especially where [cAdvisor mis-reports memory](/why/cadvisor-arm64-zero-memory). If you need host, process, and hardware metrics too and the footprint is acceptable, **cAdvisor** is the broader tool. They aren't competitors — they cover different scope.

## Decision table

| Dimension | docker-exporter | cAdvisor | Why it matters |
| --- | --- | --- | --- |
| Scope | Docker containers only | Containers + host + processes + hardware | Match the tool to what you actually graph. |
| Image size | **~9 MB** | ~250 MB | Pull time and disk on an SBC. |
| RAM (idle, ~10 containers) | **~7–10 MiB** | ~80–150 MiB | On a Pi, cAdvisor can cost more RAM than the apps you monitor. |
| CPU (steady state) | **< 1%** | 9–17% sustained (project-reported) | A constant tax vs. near-zero between scrapes. |
| cgroup v2 working set on ARM64 | **Correct** | Reports **zero** — [#3469](https://github.com/google/cadvisor/issues/3469), closed "not planned" | Broken memory dashboards on Raspberry Pi 5. |
| Privileged mode | **No** (socket read-only) | Yes | Blast radius and host-hardening posture. |
| Bind mounts | 1 (socket, `:ro`) | 6 (rootfs, /var/run, /sys, /var/lib/docker, /dev/disk, /dev/kmsg) | Setup complexity and attack surface. |
| Metric names | cAdvisor-compatible | — | Most container dashboards swap unchanged (core runtime metrics share cAdvisor names; a few, e.g. memory limit, differ). |
| Built-in UI | No (use Grafana) | Yes | cAdvisor has a standalone web UI. |

## Which should I pick for a Raspberry Pi / homelab?

- **Homelab / Raspberry Pi / SBC cluster, already on Prometheus + Grafana** → **docker-exporter**. Correct ARM64 memory, tiny footprint, no privileged mode.
- **You need host + per-process + OOM + hardware metrics** → **cAdvisor** (or run both: docker-exporter for correct container memory, cAdvisor scoped to the host metrics you actually use).
- **No Prometheus stack yet, want an all-in-one dashboard** → neither is ideal; look at a monitoring hub like Beszel or Netdata. docker-exporter assumes you already scrape Prometheus.

## When should I keep cAdvisor instead?

cAdvisor is a mature, widely deployed Google project, and it does things docker-exporter deliberately skips:

- **Host and per-process metrics**, OOM-kill events, and hardware counters.
- A **standalone web UI** with no Grafana required.
- **Non-Docker container runtimes** (containerd or CRI-O without Docker).
- A large community and years of dashboards and integrations.

If those matter to you and its footprint is fine on your host, keep it.

## FAQ

### Is docker-exporter a drop-in replacement for cAdvisor?

For per-container Docker metrics, largely yes: docker-exporter uses cAdvisor-compatible metric names (`container_cpu_usage_seconds_total`, `container_memory_working_set_bytes`, etc.), so most Prometheus scrape configs and Grafana dashboards work unchanged. It does not replace cAdvisor's host, process, or hardware metrics.

### Why does cAdvisor report zero memory on Raspberry Pi 5?

Two independent problems stack. Raspberry Pi OS ships with memory cgroups disabled by default, and even after you enable them, cAdvisor still reports zero on ARM64 with cgroup v2 — upstream issue [#3469](https://github.com/google/cadvisor/issues/3469), closed "not planned". docker-exporter computes the working set itself and reports correct memory. [Full breakdown →](/why/cadvisor-arm64-zero-memory)

## Bottom line

docker-exporter is purpose-built for one job: correct, low-footprint, per-container Docker metrics — and to fix cAdvisor's [zero-memory bug on ARM64 + cgroup v2](/why/cadvisor-arm64-zero-memory). It won't monitor your host or processes. Within its scope, on a small ARM host, it's the lighter choice, and it reports container memory correctly.

[Get started →](/guide/installation) · [Footprint benchmark →](/why/benchmark) · [Metrics reference →](/guide/metrics)
