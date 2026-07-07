---
title: Install docker-exporter on Raspberry Pi 5 / ARM64
description: "Install docker-exporter on Raspberry Pi 5 / ARM64: docker run, Docker Compose, and fixing Docker socket permission errors for the non-root UID."
---

# Installing docker-exporter on Raspberry Pi 5 / ARM64

**Two ways to run it** — `docker run` or Docker Compose — plus a one-line Docker-group GID fix when the socket needs group access (the common case on Raspberry Pi and other hardened hosts).

The image is published to GHCR for `linux/amd64` and `linux/arm64`:

```
ghcr.io/dlepaux/docker-exporter:latest
```

Pin a specific version (e.g. `:1.4.0`) in production rather than `:latest`.

## docker run

```bash
docker run -d \
  --name docker-exporter \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 \
  --restart unless-stopped \
  ghcr.io/dlepaux/docker-exporter:latest
```

The `:ro` on the socket mount is deliberate — docker-exporter only ever reads.

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

## Fixing Docker socket permission errors on Raspberry Pi {#socket-permissions}

docker-exporter runs as **UID 65532** (the distroless `nonroot` user). The Docker socket is normally owned by `root:docker`, so the container needs the host's `docker` group GID to read it:

```bash
docker run -d \
  --name docker-exporter \
  --group-add "$(getent group docker | cut -d: -f3)" \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 \
  ghcr.io/dlepaux/docker-exporter:latest
```

If the exporter exits immediately or `/metrics` shows `docker_exporter_up 0`, a missing `docker` group GID is the usual cause — see [Troubleshooting](/guide/troubleshooting#up-zero).

::: tip Rootless Docker
Rootless Docker exposes the socket at `$XDG_RUNTIME_DIR/docker.sock`, not `/var/run/docker.sock` — see [Docker's rootless mode docs](https://docs.docker.com/engine/security/rootless/). Mount that path instead.
:::

## Verify

```bash
curl -s http://localhost:9713/metrics | head
curl -s http://localhost:9713/health   # -> 200 ok
```

If `container_memory_working_set_bytes` reads zero in `/metrics`, that's a host cgroup-v2 / Raspberry Pi config issue (memory cgroups disabled by default), not this exporter — see [Why cAdvisor shows zero memory on Raspberry Pi 5](/why/cadvisor-arm64-zero-memory).

`docker ps` also shows a health status — the image's `HEALTHCHECK` runs the binary with `--health`, a TCP liveness probe to its own port on loopback (no shell, no wget). It only confirms the server is listening, though; the `/health` endpoint is what verifies the Docker daemon is reachable.

## Next

- [Configuration](/guide/configuration) — bind address, log level, container exclusion.
- [Prometheus & Grafana](/guide/prometheus-grafana) — scrape configuration and dashboards.
- [Why cAdvisor shows zero memory on Raspberry Pi 5](/why/cadvisor-arm64-zero-memory) — if you're migrating from cAdvisor and memory metrics look wrong.
- [docker-exporter vs cAdvisor](/compare/cadvisor) — footprint and correctness comparison if you're evaluating both.
