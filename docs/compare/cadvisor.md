---
title: docker-exporter vs cAdvisor — ARM64 / Raspberry Pi 5
description: docker-exporter vs cAdvisor on Raspberry Pi 5 / ARM64 — image size, RAM, CPU, privileged mode and scope, measured side by side and compared.
head:
  - - script
    - type: application/ld+json
    - |-
      {"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"Is docker-exporter a drop-in replacement for cAdvisor?","acceptedAnswer":{"@type":"Answer","text":"For per-container Docker metrics, largely yes: docker-exporter uses cAdvisor-compatible metric names (container_cpu_usage_seconds_total, container_memory_working_set_bytes, etc.), so most Prometheus scrape configs and Grafana dashboards work unchanged. It does not replace cAdvisor's host, process, or hardware metrics."}},{"@type":"Question","name":"Does cAdvisor report zero memory on a Raspberry Pi 5?","acceptedAnswer":{"@type":"Answer","text":"Only while memory cgroups are disabled, which is the Raspberry Pi default, and then docker stats and docker-exporter read zero too. With cgroup_enable=memory set, cAdvisor v0.55.1 and v0.60.6 reported the same working set as docker-exporter on a Raspberry Pi 5 (tested 2026-09-28)."}}]}
---

# docker-exporter vs cAdvisor on ARM64 / Raspberry Pi 5

**Which should you run to get Docker container metrics into Prometheus on ARM64 / Raspberry Pi 5?** If you already run Prometheus + Grafana and want low-footprint per-container metrics without a privileged container, use **docker-exporter**, a lightweight alternative to cAdvisor. If you need host, process, and hardware metrics too and the footprint is acceptable, **cAdvisor** is the broader tool. They aren't competitors — they cover different scope.

## Decision table

| Dimension | docker-exporter | cAdvisor | Why it matters |
| --- | --- | --- | --- |
| Scope | Docker containers only | Containers + host + processes + hardware | Match the tool to what you actually graph. |
| Image size (arm64) | **10.3 MB** (3.4 MB to pull) | 63.7 MB (27.7 MB to pull) | Pull time and disk on an SBC. |
| RAM (same Pi 5, 4 containers) | **7.4 MiB** | 19–29 MiB | Headroom on a small host. |
| CPU (same Pi 5, share of one core) | **0.10%** | 1.07%, and 9–17% on the project's busier hosts | A constant tax vs. near-zero between scrapes. |
| Privileged mode | **No** (socket read-only) | Yes | Blast radius and host-hardening posture. |
| Host access | the Docker socket, `:ro` | 5 bind mounts (rootfs, /var/run, /sys, /var/lib/docker, /dev/disk) plus /dev/kmsg | Setup complexity and attack surface. |
| Metric names | cAdvisor-compatible | — | Most container dashboards swap unchanged: core runtime metrics share cAdvisor names. A few differ, such as memory limit and health, which is `container_health_status` with a `status` label where cAdvisor has `container_health_state`. |
| Built-in UI | No (use Grafana) | Yes | cAdvisor has a standalone web UI. |

## Which should I pick for a Raspberry Pi / homelab?

- **Homelab / Raspberry Pi / SBC cluster, already on Prometheus + Grafana** → **docker-exporter**. The same container memory numbers, a tenth of the CPU, no privileged mode.
- **You need host + per-process + OOM + hardware metrics** → **cAdvisor** (or run both: docker-exporter for container metrics, cAdvisor scoped to the host metrics you actually use).
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

### Does cAdvisor report zero memory on a Raspberry Pi 5?

Only while memory cgroups are disabled, which is the Raspberry Pi default, and then `docker stats` and docker-exporter read zero too. With `cgroup_enable=memory` set, cAdvisor v0.55.1 and v0.60.6 reported the same working set as docker-exporter on a Raspberry Pi 5 (tested 2026-09-28). [The cause and the fix →](/why/cadvisor-arm64-zero-memory)

## Bottom line

docker-exporter is purpose-built for one job: low-footprint, per-container Docker metrics. It won't monitor your host or processes. Within its scope, on a small ARM host, it's the lighter choice.

[Get started →](/guide/installation) · [Footprint benchmark →](/why/benchmark) · [Metrics reference →](/guide/metrics)
