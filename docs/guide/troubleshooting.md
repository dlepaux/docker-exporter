---
title: Troubleshooting docker-exporter on Raspberry Pi 5 & ARM64
description: "Troubleshoot docker-exporter on Raspberry Pi 5 / ARM64 + cgroup v2: docker_exporter_up 0, socket permission denied, zero memory, and slow scrapes."
---

# Troubleshooting docker-exporter on Raspberry Pi 5 & ARM64

Most problems here are one of four things — and on ARM64 / Raspberry Pi 5, **memory reading zero** is the big one. Either the exporter won't start (socket not mounted or permission denied), a container is missing from the output, memory differs from `docker stats` or reads zero (a disabled-cgroup issue on the host, not a docker-exporter bug), or scrapes run slow. Jump to the one you're hitting.

## The exporter exits, or shows `docker_exporter_up 0` {#up-zero}

The exporter calls `docker.ping()` at startup and exits non-zero if it fails. Three common causes:

- **Socket not mounted.** Confirm the run command includes `-v /var/run/docker.sock:/var/run/docker.sock:ro` and that the host path exists. Rootless Docker uses `$XDG_RUNTIME_DIR/docker.sock` instead.
- **Permission denied.** The container runs as UID 65532 (distroless `nonroot`); the socket is normally `root:docker`. Add the `docker` group's GID: `--group-add "$(getent group docker | cut -d: -f3)"`, or run on a host where the socket GID matches. See [Installation → Socket permissions](/guide/installation#socket-permissions).
- **Daemon not on a Unix socket.** bollard uses `connect_with_socket_defaults()` — Unix socket only. A TCP-only daemon won't be reached.

## Why doesn't a container appear in metrics?

- **Excluded by pattern.** `EXCLUDE_CONTAINERS` matches on the container name (without the leading `/`); values are comma-separated and glob-aware. A pattern like `cache-*` can catch it unintentionally — plain names match exactly. See [Configuration](/guide/configuration#excluding-containers).
- **Not scraped yet.** The container was created after the last scrape and Prometheus hasn't pulled yet — wait one scrape interval.
- **Daemon not reporting it.** Stopped containers **do** appear, with `container_state{...} 0` and zero stats. If you see nothing at all, the daemon isn't returning it from `/containers/json?all=true`.

## Why does memory differ from `docker stats`?

`docker stats` shows the working set — `usage − inactive_file` on cgroup v2 (Raspberry Pi 5 / ARM64), `usage − cache` on cgroup v1. docker-exporter exposes both the raw and adjusted numbers:

- `container_memory_usage_bytes` exposes the **raw** usage including cache.
- `container_memory_working_set_bytes` matches what `docker stats` reports.

Any remaining difference after that is usually one scrape window of drift. If working set reads **zero** on a Raspberry Pi, memory cgroups are disabled on the host — the default on Raspberry Pi OS. docker-exporter reads the Docker stats API and computes the working set itself, so it reports zero only when the kernel isn't accounting memory at all, never because of an exporter bug; add `cgroup_enable=memory` and reboot to fix it. cAdvisor carries a *separate* defect on top of this — [cAdvisor #3469, "Memory Usage always zero"](https://github.com/google/cadvisor/issues/3469), closed "not planned" on 2025-12-09 — which persists even after cgroups are enabled. That second layer is why this exporter exists: see the [full ARM64 zero-memory breakdown](/why/cadvisor-arm64-zero-memory) and the [docker-exporter vs cAdvisor comparison](/compare/cadvisor).

## Scrape duration is high (> 3 s)

The bottleneck is the Docker daemon, not the exporter. Containers under heavy I/O sometimes block on stats — the 5 s per-container timeout caps individual stalls. If you consistently see 4–5 s scrapes, the daemon is overloaded; check `dockerd` CPU and disk pressure.

## Still stuck?

Open an issue on [GitHub](https://github.com/dlepaux/docker-exporter/issues) with your `docker run`/Compose config, host arch, and `LOG_LEVEL=debug` output.
