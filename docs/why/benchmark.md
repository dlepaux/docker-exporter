---
title: "ARM64/Raspberry Pi 5 benchmark: docker-exporter vs cAdvisor footprint"
description: "Measured side by side on a Raspberry Pi 5: docker-exporter used 0.13% of a core and 4.2 MiB of RAM, cAdvisor v0.60.6 used 0.87 to 1.72% and 22 to 36 MiB. Method, the --disable_metrics trap and production numbers inside."
---

# ARM64/Raspberry Pi 5 footprint benchmark: docker-exporter vs cAdvisor

**Short answer:** on the same Raspberry Pi 5, over the same 15 minutes, docker-exporter used **0.13% of a core and 4.2 MiB of RAM**. cAdvisor v0.60.6 as most people run it, on its defaults with `--docker_only=true`, used **1.72% and 35.6 MiB**. Tuned to a 15 s housekeeping interval it used 0.87% and 22.5 MiB. That is 7 to 13 times the CPU and 5 to 8 times the RAM. The gap comes from design: cAdvisor keeps walking the host's cgroups on a timer, while docker-exporter only works when Prometheus scrapes it.

## Same host, same time

Measured on 2026-09-28, 20:49:30 to 21:04:30 UTC, on a Raspberry Pi 5 (8 GB) running Raspberry Pi kernel 6.18.50, Docker 29.8.1 and cgroup v2 with `cgroup_enable=memory`. Three cAdvisor v0.60.6 containers ran side by side with docker-exporter 1.6.0, seven containers in all on the Docker daemon. Each exporter was scraped every 15 s: docker-exporter by Prometheus, each cAdvisor by a curl loop. CPU is the average over the window, as a share of one core; RAM is the working set.

| Exporter | CPU | RAM now | RAM peak | Series | Payload / scrape time |
| --- | --- | --- | --- | --- | --- |
| **docker-exporter 1.6.0** | **0.13%** | **4.2 MiB** | **6.6 MiB** | 98 | 19 KB / 2.08 s |
| cAdvisor, defaults + `--docker_only=true` (housekeeping 1 s, dynamic) | 1.72% | 35.6 MiB | 36.1 MiB | 1,337 | 1.9 MB / 0.05 s |
| cAdvisor, tuned: defaults + `--housekeeping_interval=15s` | 0.87% | 22.5 MiB | 27.4 MiB | 1,322 | 1.9 MB / 0.06 s |
| cAdvisor, tuned + `--disable_metrics=referenced_memory,percpu` | 1.47% | 34.4 MiB | 41.7 MiB | 2,495 | 3.7 MB / 0.09 s |

The scrape times go the other way, and that's expected. docker-exporter collects when it's scraped, so a scrape waits about 2 s for the Docker stats API. cAdvisor serves a cache that its background loop keeps filling, so a scrape returns at once. Both fit well inside a 15 s interval.

| Dimension | docker-exporter 1.6.0 | cAdvisor v0.60.6 |
| --- | --- | --- |
| Image, arm64 | **10.3 MB** unpacked, 3.4 MB to pull | 63.7 MB unpacked, 27.7 MB to pull |
| Privileged mode | **No** | Yes |
| Host access | the Docker socket, read-only | 5 bind mounts (`/`, `/var/run`, `/sys`, `/var/lib/docker`, `/dev/disk`) plus the `/dev/kmsg` device |

Once the kernel accounts memory, both report the same container working set: see [the cAdvisor zero-memory test](/why/cadvisor-arm64-zero-memory).

## The `--disable_metrics` trap

The last row is the configuration this project's own hosts ran in April 2026, and it costs more than the tuned row it was meant to trim. `--disable_metrics` **replaces** cAdvisor's default list of disabled collectors instead of adding to it. The defaults disable `memory_numa`, `tcp`, `udp`, `advtcp`, `sched`, `process`, `hugetlb`, `referenced_memory`, `cpu_topology`, `resctrl` and `cpuset`. Passing `--disable_metrics=referenced_memory,percpu` switches ten of them back on, and adds families like `container_network_tcp_usage_total`, `container_network_udp_usage_total`, `container_processes`, `container_sockets`, `container_threads` and `container_cpu_schedstat_*`: almost twice the series and payload.

To disable `percpu` without re-enabling the rest, repeat the defaults:

```bash
--disable_metrics=advtcp,cpu_topology,cpuset,hugetlb,memory_numa,percpu,process,referenced_memory,resctrl,sched,tcp,udp
```

## In production

- **docker-exporter** on 9 hosts running up to 36 containers used 2.0 to 10.3 MiB and 0.08 to 0.54% of a core per host (7-day medians, September 2026), peaking at 14.1 MiB.
- **cAdvisor** on this project's hosts in April 2026, before docker-exporter replaced it, used 36 to 94 MB and averaged 9 to 17% CPU over 24 hours.

Read the April figures as an upper bound, not a benchmark. They were taken with the `--disable_metrics` flag above, mostly while housekeeping still ran every 10 s, on busy hosts (33 containers on one, hundreds of seeding torrents on another), with an older cAdvisor. The flag alone costs 70% more CPU in the test above; how much more it cost on those hosts wasn't measured.

## Why the CPU gap is structural

The common cAdvisor workaround, `--docker_only=true` with a longer `--housekeeping_interval`, halves the CPU but doesn't remove it: the tuned row above uses both. Even with `docker_only`, cAdvisor keeps its housekeeping loop over the host's cgroups running, by design. Its own README pitches running "a single cAdvisor to monitor the whole machine". Users have long reported this CPU cost, as in [#2523](https://github.com/google/cadvisor/issues/2523) and [#1897](https://github.com/google/cadvisor/issues/1897).

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

Report the container count, host architecture and cAdvisor flags alongside the numbers: they're the variables that move the results most.

## What this means in practice

On a Pi or SBC running Prometheus and Grafana, cAdvisor costs 7 to 13 times the CPU and 5 to 8 times the RAM for the same container metrics, and it needs a privileged container to get them. docker-exporter gives up host and process scope for that footprint. If you *need* host or process metrics, cAdvisor is the right tool: see the [full docker-exporter vs cAdvisor comparison](/compare/cadvisor).
