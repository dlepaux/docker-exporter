---
title: Configuration
description: "Configure docker-exporter for ARM64 / Raspberry Pi 5 homelabs via env vars: LISTEN_ADDR, LOG_LEVEL, EXCLUDE_CONTAINERS globs, RUST_LOG — all optional."
---

# Configuration

Every variable below is optional — docker-exporter runs with sensible defaults out of the box.

## What environment variables does docker-exporter support?

| Variable             | Default        | Description                                                    |
| -------------------- | -------------- | -------------------------------------------------------------- |
| `LISTEN_ADDR`        | `0.0.0.0:9713` | Address and port for the HTTP server.                          |
| `LOG_LEVEL`          | `info`         | Verbosity: `trace`, `debug`, `info`, `warn`, `error`.          |
| `EXCLUDE_CONTAINERS` | *(empty)*      | Comma-separated glob patterns of container names to exclude.   |
| `RUST_LOG`           | *(unset)*      | Standard `tracing` filter — overrides `LOG_LEVEL` when set.    |

A malformed `LISTEN_ADDR`, `LOG_LEVEL`, or `EXCLUDE_CONTAINERS` value fails **loudly at startup** (non-zero exit) rather than being silently ignored. `RUST_LOG` is the exception: an invalid filter directive is ignored and logging falls back to `LOG_LEVEL`.

`RUST_LOG` takes module-scoped `tracing` directives — e.g. `RUST_LOG=docker_exporter=debug,bollard=info` turns docker-exporter's own logs up to `debug` while keeping the `bollard` Docker client at `info`.

## Excluding containers

`EXCLUDE_CONTAINERS` matches against the container **name** (without the leading `/`). Each comma-separated entry is a [glob pattern](https://docs.rs/globset):

- `*` matches any run of characters
- `?` matches a single character
- `[abc]` matches a character class

Patterns are **whole-name anchored**, so `cache-*` excludes `cache-redis` but not `app-cache`. A plain name with no wildcards (`prometheus`) is an **exact** match — so existing values keep working, no migration needed.

```bash
EXCLUDE_CONTAINERS=prometheus,cadvisor,debug-*,*-sidecar
```

Running docker-exporter alongside cAdvisor during a migration? Exclude cAdvisor's own container the same way — see the [docker-exporter vs cAdvisor comparison](/compare/cadvisor).

The same fail-fast rule applies here: an unclosed `[`, for example, is rejected at startup, not silently dropped.

## Example (Compose)

```yaml
services:
  docker-exporter:
    image: ghcr.io/dlepaux/docker-exporter:latest
    restart: unless-stopped
    environment:
      LOG_LEVEL: warn
      EXCLUDE_CONTAINERS: "prometheus,grafana,*-sidecar"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    ports:
      - "9713:9713"
```

## Next

- [Metrics reference](/guide/metrics) — every metric `LOG_LEVEL` and `EXCLUDE_CONTAINERS` affect.
- [Prometheus & Grafana](/guide/prometheus-grafana) — scrape docker-exporter and visualize it.
