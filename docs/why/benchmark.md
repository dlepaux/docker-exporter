---
title: "ARM64 benchmark: docker-exporter vs cAdvisor footprint on a Raspberry Pi 5 and a busy Rock 5B+"
description: "Measured side by side on two ARM64 hosts: docker-exporter used 7 to 30 times less CPU than cAdvisor v0.60.6 and 5 to 84 times less RAM. Charts, method and the --disable_metrics trap inside."
---

<script setup>
const pi = {
  cpu: [
    { label: "docker-exporter 1.6.0", value: 0.13, highlight: true },
    { label: "cAdvisor, defaults", value: 1.72 },
    { label: "cAdvisor, 15 s housekeeping", value: 0.87 },
  ],
  ram: [
    { label: "docker-exporter 1.6.0", value: 4.2, highlight: true },
    { label: "cAdvisor, defaults", value: 35.6 },
    { label: "cAdvisor, 15 s housekeeping", value: 22.5 },
  ],
};
const rock = {
  cpu: [
    { label: "docker-exporter 1.6.0", value: 0.67, highlight: true },
    { label: "cAdvisor, defaults", value: 20.36 },
    { label: "cAdvisor, 15 s housekeeping", value: 10.53 },
  ],
  ram: [
    { label: "docker-exporter 1.6.0", value: 5.1, highlight: true },
    { label: "cAdvisor, defaults", value: 428.6 },
    { label: "cAdvisor, 15 s housekeeping", value: 127.9 },
  ],
};
const trap = [
  { label: "15 s housekeeping", value: 10.53 },
  { label: "+ socket collectors (tcp, udp, advtcp)", value: 16.61 },
  { label: "+ disable_metrics=referenced_memory,percpu", value: 17.77 },
];
</script>

# ARM64 footprint benchmark: docker-exporter vs cAdvisor

**Short answer:** measured side by side on two ARM64 hosts, docker-exporter used **7 to 30 times less CPU** than cAdvisor v0.60.6 and **5 to 84 times less RAM**. The gap widens with the host's load. On a quiet Raspberry Pi 5 docker-exporter used 0.13% of a core and 4.2 MiB, and on a busy Rock 5B+ running 37 containers, 0.67% and 5.1 MiB. cAdvisor on that busy host used 10.5 to 20.4% of a core and 128 to 429 MiB. The difference comes from design: cAdvisor keeps walking the host's cgroups on a timer, while docker-exporter only works when Prometheus scrapes it.

## Setup

Every run put docker-exporter and several cAdvisor v0.60.6 containers on the same host, over the same 15 minutes, each scraped every 15 s: docker-exporter by Prometheus, each cAdvisor by a curl loop. cAdvisor ran from its README command plus `--docker_only=true`, which is how most homelabs run it. CPU is the average over the window, as a share of one core. RAM is the container's working set at the end of the window, with the window's peak in the tables.

The configurations:

- **defaults**: cAdvisor's own housekeeping, every 1 s with dynamic back-off;
- **15 s housekeeping**: `--housekeeping_interval=15s`, the usual tuning advice;
- two more in [the `--disable_metrics` trap](#the-disable-metrics-trap) below.

## Quiet host: Raspberry Pi 5

Raspberry Pi 5, 8 GB, Raspberry Pi kernel 6.18.50, Docker 29.8.1, cgroup v2 with `cgroup_enable=memory`. Seven containers, the exporters included. 2026-09-28, 20:49:30 to 21:04:30 UTC.

<FootprintChart title="CPU, share of one core" unit="%" :digits="2" :rows="pi.cpu" />
<FootprintChart title="RAM, working set" unit=" MiB" :rows="pi.ram" />

| Exporter | CPU | RAM (peak) | Series | Payload / scrape time |
| --- | --- | --- | --- | --- |
| **docker-exporter 1.6.0** | **0.13%** | **4.2 MiB** (6.6) | 98 | 19 KB / 2.08 s |
| cAdvisor, defaults | 1.72% | 35.6 MiB (36.1) | 1,337 | 1.9 MB / 0.05 s |
| cAdvisor, 15 s housekeeping | 0.87% | 22.5 MiB (27.4) | 1,322 | 1.9 MB / 0.06 s |

## Busy host: Rock 5B+

Radxa Rock 5B+, 8 cores, 24 GB, vendor kernel 6.1.115, Docker 29.8.0, cgroup v2. It runs a media stack: 37 containers, including a torrent client holding about 1,800 TCP sockets, plus the four cAdvisors under test. 2026-09-28, 22:27:30 to 22:42:30 UTC.

<FootprintChart title="CPU, share of one core" unit="%" :digits="2" :rows="rock.cpu" />
<FootprintChart title="RAM, working set" unit=" MiB" :rows="rock.ram" />

| Exporter | CPU | RAM (peak) | Series | Payload / scrape time |
| --- | --- | --- | --- | --- |
| **docker-exporter 1.6.0** | **0.67%** | **5.1 MiB** (5.4) | 577 | 107 KB / 2.62 s |
| cAdvisor, defaults | 20.36% | 428.6 MiB (484.3) | 4,183 | 8.8 MB / 0.32 s |
| cAdvisor, 15 s housekeeping | 10.53% | 127.9 MiB (155.8) | 4,198 | 8.8 MB / 0.35 s |

cAdvisor's cost grows with the number of containers and how busy they are. docker-exporter's stays near flat: from 7 to 41 containers on the host it went from 0.13% to 0.67% of a core, and from 4.2 to 5.1 MiB.

The scrape times go the other way, and that's expected. docker-exporter collects when it's scraped, so a scrape waits 2 to 3 s for the Docker stats API. cAdvisor serves a cache that its background loop keeps filling, so a scrape returns in well under a second. Both fit inside a 15 s interval.

## Image and privileges

| Dimension | docker-exporter 1.6.0 | cAdvisor v0.60.6 |
| --- | --- | --- |
| Image, arm64 | **10.3 MB** unpacked, 3.4 MB to pull | 63.7 MB unpacked, 27.7 MB to pull |
| Privileged mode | **No** | Yes |
| Host access | the Docker socket, read-only | 5 bind mounts (`/`, `/var/run`, `/sys`, `/var/lib/docker`, `/dev/disk`) plus the `/dev/kmsg` device |

Once the kernel accounts memory, both report the same container working set: see [the cAdvisor zero-memory test](/why/cadvisor-arm64-zero-memory).

## The `--disable_metrics` trap

`--disable_metrics` **replaces** cAdvisor's default list of disabled collectors instead of adding to it. The defaults disable `advtcp`, `cpu_topology`, `cpuset`, `hugetlb`, `memory_numa`, `process`, `referenced_memory`, `resctrl`, `sched`, `tcp` and `udp`. So `--disable_metrics=referenced_memory,percpu`, which reads like a way to trim cAdvisor, switches ten collectors back on. Measured on the busy host, each run on top of 15 s housekeeping:

<FootprintChart title="cAdvisor CPU on the Rock 5B+, share of one core" unit="%" :digits="2" :rows="trap" />

| cAdvisor configuration | CPU | Series | Payload per scrape |
| --- | --- | --- | --- |
| 15 s housekeeping | 10.53% | 4,198 | 8.8 MB |
| + only the socket collectors (`tcp`, `udp`, `advtcp`) re-enabled | 16.61% | 9,646 | 21.4 MB |
| + `--disable_metrics=referenced_memory,percpu` | 17.77% | 10,552 | 23.5 MB |

That flag cost about 70% more CPU on both hosts (0.87% to 1.47% on the Pi 5), and multiplied the series count by 1.9 to 2.5. On the busy host the socket collectors alone account for most of it. To disable `percpu` without re-enabling the rest, repeat the defaults:

```bash
--disable_metrics=advtcp,cpu_topology,cpuset,hugetlb,memory_numa,percpu,process,referenced_memory,resctrl,sched,tcp,udp
```

## In production

docker-exporter on 9 hosts running up to 36 containers used 2.0 to 10.3 MiB and 0.08 to 0.54% of a core per host (7-day medians, September 2026), peaking at 14.1 MiB.

## Why the CPU gap is structural

The common cAdvisor tuning, `--docker_only=true` with a longer `--housekeeping_interval`, halves its CPU but doesn't remove it: the 15 s rows above use both. Even with `docker_only`, cAdvisor keeps its housekeeping loop over the host's cgroups running, by design. Its own README pitches running "a single cAdvisor to monitor the whole machine". Users have long reported this CPU cost, as in [#2523](https://github.com/google/cadvisor/issues/2523) and [#1897](https://github.com/google/cadvisor/issues/1897).

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

On a Pi or SBC running Prometheus and Grafana, cAdvisor costs 7 to 30 times the CPU and 5 to 84 times the RAM for the same container metrics, more the busier the host, and it needs a privileged container to get them. docker-exporter gives up host and process scope for that footprint. If you *need* host or process metrics, cAdvisor is the right tool: see the [full docker-exporter vs cAdvisor comparison](/compare/cadvisor).
