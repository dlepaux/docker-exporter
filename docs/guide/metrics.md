---
title: Docker metrics reference (ARM64 + cgroup v2)
description: "Every docker-exporter metric for ARM64 + cgroup v2 hosts: CPU, memory working set, network, disk I/O, state and health — with types, labels, examples."
---

# Docker metrics reference (ARM64 + cgroup v2)

docker-exporter exposes per-container CPU, memory, network, disk I/O, state, and health as Prometheus text at `/metrics` — including `container_memory_working_set_bytes` computed correctly on ARM64 + cgroup v2 (Raspberry Pi 5). Metric names are cAdvisor-compatible, so existing Grafana dashboards work unchanged.

All per-container metrics carry the base labels `id`, `image`, and `name`. Some add extra labels, noted below.

## Exporter

| Metric                                    | Type    | Labels | Description                                          |
| ----------------------------------------- | ------- | ------ | ---------------------------------------------------- |
| `docker_exporter_up`                      | gauge   | —      | Docker daemon reachable (`1` = up, `0` = down).      |
| `docker_exporter_scrape_duration_seconds` | gauge   | —      | Duration of the last scrape, in seconds.             |
| `docker_exporter_inspect_failures_total`  | counter | —      | Total container-inspect failures since start.        |
| `docker_exporter_stats_failures_total`    | counter | —      | Total container-stats fetch failures since start.    |

Alert on both failure counters. `docker_exporter_up` only tells you the daemon
answered the container *list* — a scrape can be `up` and still be lying. When an
inspect fails, that container's `health` and `restart_policy` read `unknown`;
when a stats fetch fails, its cpu/memory/network series report `0` rather than
disappearing, which is indistinguishable from a genuinely idle container.

```promql
rate(docker_exporter_inspect_failures_total[5m]) > 0
rate(docker_exporter_stats_failures_total[5m]) > 0
```

## CPU

| Metric                              | Type    | Labels                | Description                     |
| ----------------------------------- | ------- | --------------------- | ------------------------------- |
| `container_cpu_usage_seconds_total` | counter | `id`, `image`, `name` | Cumulative CPU usage in seconds. |

## Memory

| Metric                               | Type  | Labels                | Description                                                  |
| ------------------------------------ | ----- | --------------------- | ------------------------------------------------------------ |
| `container_memory_usage_bytes`       | gauge | `id`, `image`, `name` | Current memory usage in bytes (**includes** cache).          |
| `container_memory_working_set_bytes` | gauge | `id`, `image`, `name` | Working set: `usage − inactive_file` (v2), `usage − cache` (v1). |
| `container_memory_cache`             | gauge | `id`, `image`, `name` | Cache: `inactive_file` (v2), `cache` (v1).          |
| `container_memory_limit_bytes`       | gauge | `id`, `image`, `name` | Memory limit in bytes.                                       |

`container_memory_working_set_bytes` is the metric cAdvisor reports as **zero** on ARM64 + cgroup v2 — a known upstream bug ([cAdvisor #3469](https://github.com/google/cadvisor/issues/3469), closed *"not planned"* 2025-12-09). It matches what `docker stats` reports. [Why cAdvisor gets it wrong →](/why/cadvisor-arm64-zero-memory)

See the [full docker-exporter vs. cAdvisor comparison →](/compare/cadvisor) for footprint and metric-coverage differences.

## Network

| Metric                                    | Type    | Labels                              | Description                          |
| ----------------------------------------- | ------- | ----------------------------------- | ------------------------------------ |
| `container_network_receive_bytes_total`   | counter | `id`, `image`, `name`, `interface`  | Cumulative network bytes received.   |
| `container_network_transmit_bytes_total`  | counter | `id`, `image`, `name`, `interface`  | Cumulative network bytes transmitted. |

## Disk I/O

| Metric                               | Type    | Labels                                | Description                                            |
| ------------------------------------ | ------- | ------------------------------------- | ------------------------------------------------------ |
| `container_blkio_device_usage_total` | counter | `id`, `image`, `name`, `operation`    | Cumulative block I/O bytes (`operation` = `read`/`write`). |

Block I/O metrics emit only after a container reports non-zero bytes — containers that never touch disk produce no `blkio` series, keeping cardinality down.

## State, health & lifecycle

| Metric                         | Type  | Labels                                              | Description                                              |
| ------------------------------ | ----- | --------------------------------------------------- | -------------------------------------------------------- |
| `container_state`              | gauge | `id`, `image`, `name`, `state`, `restart_policy`    | `1` if `state == "running"`, `0` otherwise.              |
| `container_exit_code`          | gauge | `id`, `image`, `name`, `restart_policy`             | Exit code of a container in a terminal state (`exited`/`dead`). |
| `container_health_status`      | gauge | `id`, `image`, `name`, `status`                     | `1` for the container's current health `status`.         |
| `container_start_time_seconds` | gauge | `id`, `image`, `name`                               | Container creation time as a Unix timestamp.             |
| `container_last_seen`          | gauge | `id`, `image`, `name`                               | Last scrape time as a Unix timestamp.                    |

- **`restart_policy`** lives only on `container_state` and `container_exit_code` — adding it to every family would multiply series counts for no benefit. It lets alerting exclude one-shot containers by label — e.g. select `container_state{restart_policy!="no"}` in a `ContainerStopped` alert instead of maintaining a name denylist.
- **`container_exit_code`** is the failure signal for one-shot containers (`restart: "no"` migrations, batch jobs). `container_state` collapses to `0` for anything not running and cannot tell `Exited(0)` from `Exited(1)`, so a job that ran and failed looks exactly like one that ran and succeeded. The value is the raw code — `137` (OOM/`SIGKILL`), `1`, `78` — because the number is the diagnosis.
  - Emitted **only** for `exited`/`dead` containers. A running container's inspect reports `ExitCode: 0`, and a `created` one has never run; publishing either would read as "succeeded".
  - **Absent, never `0`, when the inspect fails** — the exporter has no exit code to report and will not invent one. `docker_exporter_inspect_failures_total` is the signal for that case.
  - The series appears when a one-shot finishes and disappears when the container is replaced, so `container_exit_code{restart_policy="no"} != 0` resolves on redeploy.
- **`container_health_status`** emits one series per container, with `status` set to the current Docker health (`healthy`, `unhealthy`, `starting`, or `none` when the container has no `HEALTHCHECK`).

## Endpoints

| Path       | Description                                       |
| ---------- | ------------------------------------------------- |
| `/metrics` | Prometheus exposition (`text/plain; version=0.0.4; charset=utf-8`). |
| `/health`  | Liveness — `200 ok` if the Docker daemon is reachable. |
| `/ready`   | Readiness — `200 ready` if the Docker daemon is reachable.   |

The image ships a `HEALTHCHECK` that runs the binary's built-in `--health` flag — a lightweight TCP liveness probe against its own listening port (no shell or `wget`, so it works on distroless) — so `docker ps` shows health out of the box. That probe only confirms the HTTP server is accepting connections; the `/health` endpoint additionally pings the Docker daemon and returns 503 if it is unreachable.

## Sample output

Trimmed to a few lines, container names anonymized:

```text
# HELP docker_exporter_up Whether the Docker daemon is reachable (1 = up, 0 = down)
# TYPE docker_exporter_up gauge
docker_exporter_up 1
# TYPE docker_exporter_scrape_duration_seconds gauge
docker_exporter_scrape_duration_seconds 2.18
# TYPE container_cpu_usage_seconds_total counter
container_cpu_usage_seconds_total{id="abc...",image="nginx:alpine",name="web-app"} 145.26
# TYPE container_memory_working_set_bytes gauge
container_memory_working_set_bytes{id="abc...",image="nginx:alpine",name="web-app"} 37371904
# TYPE container_state gauge
container_state{id="abc...",image="nginx:alpine",name="web-app",state="running",restart_policy="unless-stopped"} 1
# TYPE container_exit_code gauge
container_exit_code{id="def...",image="myapp/migrate:1.4",name="db-migrate",restart_policy="no"} 1
# TYPE container_health_status gauge
container_health_status{id="abc...",image="nginx:alpine",name="web-app",status="healthy"} 1
```

## Stability

Metric names and labels follow SemVer. Any breaking change to a metric name or label requires a major version bump — none are planned.

## Next

- [Prometheus & Grafana](/guide/prometheus-grafana) — scrape these metrics and see them in Grafana.
- [Troubleshooting](/guide/troubleshooting) — when a metric is missing or reads zero.
