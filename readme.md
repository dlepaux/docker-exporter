![CI](https://github.com/dlepaux/docker-exporter/actions/workflows/ci.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Image size](https://ghcr-badge.egpl.dev/dlepaux/docker-exporter/size)

# docker-exporter

A Prometheus exporter for Docker container metrics, built in Rust for ARM64 homelabs running cgroup v2.

cAdvisor returns zero for `container_memory_working_set_bytes` and `container_memory_rss` on Raspberry Pi 5 — same cAdvisor build, same cgroup v2 + arm64 combination, no issue on other ARM64 SBCs (e.g. Rock 5B+). Memory dashboards silently lie. `docker-exporter` is a single-binary alternative that reads the Docker stats API directly, computes the working set correctly on both cgroup versions, talks to the socket read-only, and runs in ~7 MiB of RAM at idle.

## Why this exists

The Pi 5 zero-metric bug isn't cAdvisor's only pain point — it also sits at 9-17% sustained CPU and 36-94 MB RAM per host, with six bind mounts and privileged mode. [cAdvisor#2523](https://github.com/google/cadvisor/issues/2523) tracks more ARM-side issues. The tool isn't sized for SBC homelabs.

Existing alternatives were dormant or amd64-only. So I wrote a Rust replacement: single self-contained binary, no GC churn, ~7 MiB RAM and <1% CPU at steady state. Read-only socket, no privileged mode, cAdvisor-compatible metric names so dashboards swap without rewrites.

## Features

- Correct memory working set on cgroup v1 **and** v2 (`usage − inactive_file` on v2, `usage − cache` on v1)
- Per-container CPU, memory, network, and disk I/O
- Read-only Docker socket; runs as non-root (UID 65532) inside the container
- On-demand collection — no background polling, stats fetched per scrape with a 5 s per-container timeout
- Container exclusion filter via env var
- `/health` and `/ready` endpoints (Docker `HEALTHCHECK` already wired in the image)

## Resource use

- **Memory**: ~7–10 MiB at idle, ~10–20 MiB scraping ~30 containers.
- **CPU**: idle between scrapes (no background loop). Per scrape, work scales with container count and Docker daemon latency — typically a small single-digit percentage on a Raspberry Pi 5.
- **Scrape duration**: usually 1–3 s, dominated by the Docker daemon's `/containers/{id}/stats` endpoint. Per-container concurrency with a 5 s timeout caps individual stalls.
- **Image**: ~9 MB (static musl binary on `distroless/static`, non-root). Same on `linux/amd64` and `linux/arm64`.

## cAdvisor — when to pick which

`docker-exporter` is purpose-built for one job. cAdvisor monitors much more (host, processes, OOM events, hardware counters). Pick whichever matches your scope.

| Dimension                          | docker-exporter        | cAdvisor                               |
| ---------------------------------- | ---------------------- | -------------------------------------- |
| Scope                              | Docker containers only | Containers + host + processes          |
| Image size                         | ~9 MB (musl + distroless)    | ~250 MB                          |
| RAM idle (small host, ~10 cont.)   | ~7–10 MiB              | ~80–150 MiB                            |
| cgroup v2 working set on ARM64     | Correct                | Reports raw RSS — known issue          |
| Privileged container required      | No (socket RO)         | Yes (mounts cgroup, proc, sys)         |
| Built-in UI                        | No                     | Yes                                    |

If you already run cAdvisor and your dashboards work, keep it. If you're hitting the cgroup-v2 memory bug or want a small footprint on an SBC, this exporter gets you back to accurate dashboards without privileged mode.

## Quick start

```bash
docker run -d \
  --name docker-exporter \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 \
  --restart unless-stopped \
  ghcr.io/dlepaux/docker-exporter:latest
```

Image is published for `linux/amd64` and `linux/arm64`.

## Docker Compose

```yaml
services:
  docker-exporter:
    image: ghcr.io/dlepaux/docker-exporter:latest
    container_name: docker-exporter
    restart: unless-stopped
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    ports:
      - "9713:9713"
```

## Configuration

All configuration is via environment variables. All are optional.

| Variable             | Default        | Description                                              |
| -------------------- | -------------- | -------------------------------------------------------- |
| `LISTEN_ADDR`        | `0.0.0.0:9713` | Address and port to bind the HTTP server                 |
| `LOG_LEVEL`          | `info`         | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `EXCLUDE_CONTAINERS` | _(empty)_      | Comma-separated container name patterns to exclude        |
| `RUST_LOG`           | _(unset)_      | Standard `tracing` filter — overrides `LOG_LEVEL` if set |

`EXCLUDE_CONTAINERS` matches against the container **name** (without the leading `/`). Each comma-separated entry is a [glob pattern](https://docs.rs/globset): `*` matches any run of characters, `?` a single character, `[abc]` a character class. Patterns are whole-name anchored, so `cache-*` excludes `cache-redis` but not `app-cache`. A plain name with no wildcards (`prometheus`) is an exact match — existing exact values keep working unchanged, no migration needed. A malformed pattern fails loudly at startup rather than being silently ignored.

```
EXCLUDE_CONTAINERS=prometheus,cadvisor,debug-*,*-sidecar
```

## Endpoints

| Path       | Description                                    |
| ---------- | ---------------------------------------------- |
| `/metrics` | Prometheus exposition (`text/plain; v=0.0.4`)  |
| `/health`  | Liveness — `200 ok` if Docker daemon reachable |
| `/ready`   | Readiness — `200 ready` if Docker reachable    |

`/health` is already wired into the image's `HEALTHCHECK`, so `docker ps` will show health status out of the box.

## Metrics

### Exporter

| Metric                                    | Type  | Labels | Description                                          |
| ----------------------------------------- | ----- | ------ | ---------------------------------------------------- |
| `docker_exporter_up`                      | gauge | —      | Docker daemon reachable (1 = up, 0 = down)           |
| `docker_exporter_scrape_duration_seconds` | gauge | —      | Duration of the last scrape in seconds               |

### CPU

| Metric                              | Type    | Labels                | Description                     |
| ----------------------------------- | ------- | --------------------- | ------------------------------- |
| `container_cpu_usage_seconds_total` | counter | `id`, `image`, `name` | Cumulative CPU usage in seconds |

### Memory

| Metric                               | Type  | Labels                | Description                                                     |
| ------------------------------------ | ----- | --------------------- | --------------------------------------------------------------- |
| `container_memory_usage_bytes`       | gauge | `id`, `image`, `name` | Current memory usage in bytes (includes cache)                  |
| `container_memory_working_set_bytes` | gauge | `id`, `image`, `name` | Working set (`usage − inactive_file` v2, `usage − cache` v1)    |
| `container_memory_cache`             | gauge | `id`, `image`, `name` | Cache (`inactive_file` on cgroup v2, `cache` on v1)             |
| `container_memory_limit_bytes`       | gauge | `id`, `image`, `name` | Memory limit in bytes                                           |

### Network

| Metric                                  | Type    | Labels                             | Description                          |
| --------------------------------------- | ------- | ---------------------------------- | ------------------------------------ |
| `container_network_receive_bytes_total` | counter | `id`, `image`, `name`, `interface` | Cumulative network bytes received    |
| `container_network_transmit_bytes_total` | counter | `id`, `image`, `name`, `interface` | Cumulative network bytes transmitted |

### Disk I/O

| Metric                               | Type    | Labels                                      | Description                                               |
| ------------------------------------ | ------- | ------------------------------------------- | --------------------------------------------------------- |
| `container_blkio_device_usage_total` | counter | `id`, `image`, `name`, `operation`          | Cumulative block I/O bytes (`operation` = `read`/`write`) |

Block I/O is only exposed when the container has reported non-zero bytes — containers that never touch disk produce no `blkio` series, keeping cardinality down.

### Container info

| Metric                         | Type  | Labels                         | Description                                       |
| ------------------------------ | ----- | ------------------------------ | ------------------------------------------------- |
| `container_state`              | gauge | `id`, `image`, `name`, `state` | 1 if `state == "running"`, 0 otherwise            |
| `container_start_time_seconds` | gauge | `id`, `image`, `name`          | Container creation time as Unix timestamp         |
| `container_last_seen`          | gauge | `id`, `image`, `name`          | Last scrape time as Unix timestamp                |

## Sample output

What `curl http://localhost:9713/metrics` looks like (trimmed to ~10 lines, container names anonymized):

```text
# HELP docker_exporter_up Whether the Docker daemon is reachable (1 = up, 0 = down)
# TYPE docker_exporter_up gauge
docker_exporter_up 1
# HELP docker_exporter_scrape_duration_seconds Duration of the last scrape in seconds
# TYPE docker_exporter_scrape_duration_seconds gauge
docker_exporter_scrape_duration_seconds 2.18
# HELP container_cpu_usage_seconds_total Cumulative CPU usage in seconds
# TYPE container_cpu_usage_seconds_total counter
container_cpu_usage_seconds_total{id="abc...",image="nginx:alpine",name="web-app"} 145.26
container_cpu_usage_seconds_total{id="def...",image="postgres:17",name="db"} 406.22
container_cpu_usage_seconds_total{id="ghi...",image="redis:7-alpine",name="cache"} 468.98
# HELP container_memory_working_set_bytes Current memory working set in bytes (usage minus cache)
# TYPE container_memory_working_set_bytes gauge
container_memory_working_set_bytes{id="abc...",image="nginx:alpine",name="web-app"} 37371904
container_memory_working_set_bytes{id="def...",image="postgres:17",name="db"} 77758464
container_memory_working_set_bytes{id="ghi...",image="redis:7-alpine",name="cache"} 5701632
```

## Prometheus

```yaml
scrape_configs:
  - job_name: docker
    static_configs:
      - targets: ["docker-exporter:9713"]
```

Default Prometheus scrape interval (15 s) is fine. Avoid going below 5 s — the Docker stats API has its own internal sampling window and scraping faster won't give you fresher data, just more load on the daemon.

## Architecture

The exporter has no background loop. On each `GET /metrics` it lists all containers (running and stopped, so `container_state` reports them all), then fetches per-container stats concurrently via `bollard::stats(stream=false)` with a 5 s timeout each. Failed or timed-out containers are logged and skipped — they don't fail the whole scrape.

Working set is computed at exposition time from the Docker stats payload: `inactive_file` on cgroup v2, `cache` on v1, with a `max(0, usage − cache)` floor. The Prometheus output is built directly as `MetricFamily` protos (no `Registry`) so counter values can be emitted as absolute numbers without state across scrapes — the process holds zero shared mutable state between requests.

Scrape duration therefore tracks the Docker daemon's `/containers/{id}/stats` latency, not the exporter — the exporter itself adds <1 ms of overhead per scrape.

## Troubleshooting

**`docker_exporter_up 0` or the container restarts immediately.**
The exporter calls `docker.ping()` at startup and exits non-zero if it fails. Three common causes:
- Socket not mounted: confirm `-v /var/run/docker.sock:/var/run/docker.sock:ro` is on the run command and the path on the host actually exists (rootless Docker uses `$XDG_RUNTIME_DIR/docker.sock` instead).
- Permission denied: the container runs as UID 65532 (distroless `nonroot`). The Docker socket is normally owned by `root:docker`. Add the `docker` group's GID to the container (`--group-add $(getent group docker | cut -d: -f3)`) or run the exporter on a host where the socket GID matches.
- Daemon isn't listening on a Unix socket (e.g. only on TCP). bollard uses `connect_with_socket_defaults()` — Unix socket only at the moment.

**A container doesn't appear in metrics.**
- Check `EXCLUDE_CONTAINERS` — the match is on the container name (without the leading `/`), comma-separated, glob-aware (a wildcard pattern like `cache-*` may be catching it; plain names match exactly).
- The container was created after the last scrape and Prometheus hasn't pulled yet — wait one scrape interval.
- Stopped containers *do* appear, with `container_state{...} 0` and zero stats. If you see nothing at all for a container, the Docker daemon isn't returning it from `/containers/json?all=true`.

**Memory differs from `docker stats`.**
`docker stats` shows `usage − cache` (working set). `container_memory_usage_bytes` here exposes the **raw** usage including cache; `container_memory_working_set_bytes` is the apples-to-apples comparison with `docker stats`. Difference between working set and `docker stats` after that is usually one scrape window of drift.

**Scrape duration is high (>3 s).**
The bottleneck is the Docker daemon, not the exporter. Containers under heavy I/O sometimes block on stats — the 5 s per-container timeout caps individual stalls. If you consistently see 4–5 s scrapes, your daemon is overloaded; check `dockerd` CPU and disk pressure.

## Versioning and compatibility

- **Docker API**: bollard 0.20 negotiates against API ≥ 1.41 (Docker 20.10+). Tested against Docker 27.x and 28.x daemons.
- **cgroup**: tested on cgroup v1 and v2. Working-set computation handles both.
- **Architectures**: `linux/amd64`, `linux/arm64`. No 32-bit ARM build (no demand; PRs welcome).
- **Rust**: pinned to 1.85 stable in [`rust-toolchain.toml`](rust-toolchain.toml).
- **Versioning**: SemVer, automated by [release-please](https://github.com/googleapis/release-please). Breaking changes to metric names or labels would be a major bump — none planned.

## Development

```bash
cargo build           # debug build
cargo test            # unit + integration tests (some require a local Docker daemon)
cargo clippy          # lints
cargo fmt             # format
cargo build --release # release binary in target/release/docker-exporter
```

Tests that need a Docker daemon skip themselves (with a stderr note) when the socket isn't reachable, so `cargo test` is safe to run anywhere.

## Acknowledgments

Built on:
- [bollard](https://github.com/fussybeaver/bollard) — Docker Engine API client for Rust
- [prometheus](https://github.com/tikv/rust-prometheus) — Prometheus client / text encoder
- [axum](https://github.com/tokio-rs/axum) — HTTP server
- [tokio](https://tokio.rs) — async runtime

Related projects:
- [cAdvisor](https://github.com/google/cadvisor) — Google's container monitor; broader scope, larger footprint, broken cgroup-v2 memory on ARM64 (the reason this exporter exists).
- [docker_stats_exporter](https://github.com/wywywywy/docker_stats_exporter) — Python equivalent; same scope, but no working-set fix and a heavier base image.

## Security

Found a vulnerability? Please follow the disclosure policy in [`SECURITY.md`](SECURITY.md) — do **not** open a public issue.

## License

[MIT](license.md)
