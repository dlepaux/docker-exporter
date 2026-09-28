---
title: "ARM64/Raspberry Pi 5 benchmark: docker-exporter vs cAdvisor footprint"
description: "Measured side by side on a Raspberry Pi 5: docker-exporter used 0.1% of a core and 7 MiB of RAM, cAdvisor v0.60.6 used 1.1% and 19 to 29 MiB. Method and production numbers inside."
---

# ARM64/Raspberry Pi 5 footprint benchmark: docker-exporter vs cAdvisor

**Short answer:** on the same Raspberry Pi 5, over the same 15 minutes, docker-exporter used **0.10% of a core and 7.4 MiB of RAM**. cAdvisor v0.60.6 used **1.07% and 19 MiB, peaking at 29 MiB**. Both reported the same container memory. The gap comes from design: cAdvisor keeps walking the host's cgroups on a timer, while docker-exporter only works when Prometheus scrapes it.

## Same host, same time

Measured on 2026-09-28 on a Raspberry Pi 5 (8 GB) running Raspberry Pi kernel 6.18, Docker 29.8.1 and cgroup v2 with `cgroup_enable=memory`. Four containers ran on the Docker daemon. Both exporters were scraped every 15 s for 15 minutes. CPU is the average over that window; RAM is the working set.

| Dimension | docker-exporter 1.6.0 | cAdvisor v0.60.6 |
| --- | --- | --- |
| CPU (share of one core) | **0.10%** | 1.07% |
| RAM (working set) | **7.4 MiB** (7.5 peak) | 19 MiB (28.8 peak) |
| Image, arm64 | **10.3 MB** unpacked, 3.4 MB to pull | 63.7 MB unpacked, 27.7 MB to pull |
| Privileged mode | **No** | Yes |
| Host access | the Docker socket, read-only | 5 bind mounts (`/`, `/var/run`, `/sys`, `/var/lib/docker`, `/dev/disk`) plus the `/dev/kmsg` device |

cAdvisor ran with its README command plus `--docker_only=true --housekeeping_interval=15s --disable_metrics=referenced_memory,percpu`, the settings this project's own hosts used.

## In production

Four containers make for a small host. On busier ones the gap grows:

- **docker-exporter** on 9 hosts running up to 36 containers used 2.0 to 10.3 MiB and 0.08 to 0.54% of a core per host (7-day medians, September 2026), peaking at 14 MiB.
- **cAdvisor** on this project's hosts in April 2026, before docker-exporter replaced it, used 36 to 94 MB and averaged 9 to 17% CPU over 24 hours, even with `housekeeping_interval` raised to 15 s.

The April figures come from an older cAdvisor on different hardware, so read them as a trend, not a benchmark.

## Why the CPU gap is structural

The common cAdvisor workaround, `--docker_only=true` with a longer `--housekeeping_interval`, reduces CPU but doesn't remove it: the test above used both. Even with `docker_only`, cAdvisor keeps its housekeeping loop over the host's cgroups running, by design. Its own README pitches running "a single cAdvisor to monitor the whole machine". Users have long reported this CPU cost, as in [#2523](https://github.com/google/cadvisor/issues/2523) and [#1897](https://github.com/google/cadvisor/issues/1897).

docker-exporter has no such floor because it does no host introspection. It has **no background loop**: work happens only during a scrape, and each scrape calls the Docker stats API. Between scrapes, CPU is idle. [Architecture →](/guide/architecture)

## Methodology (reproduce it)

Run both exporters on the same host and scrape both every 15 s, the way Prometheus would:

```bash
while true; do
  curl -s localhost:9713/metrics >/dev/null   # docker-exporter
  curl -s localhost:8080/metrics >/dev/null   # cAdvisor
  sleep 15
done &
```

After 15 minutes, read both from docker-exporter's own series, which cover every container on the host, the two exporters included:

```promql
# CPU, average share of one core over the window
100 * rate(container_cpu_usage_seconds_total{name=~"docker-exporter|cadvisor"}[15m])

# RAM, working set (docker stats shows the same number)
container_memory_working_set_bytes{name=~"docker-exporter|cadvisor"}
```

Report the container count and host architecture alongside the numbers: they're the variables that move the results most.

## What this means in practice

On a Pi or SBC running Prometheus and Grafana, cAdvisor costs about ten times the CPU for the same container metrics, and it needs a privileged container to get them. docker-exporter gives up host and process scope for that footprint. If you *need* host or process metrics, cAdvisor is the right tool: see the [full docker-exporter vs cAdvisor comparison](/compare/cadvisor).
