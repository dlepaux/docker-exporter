---
title: Security
description: docker-exporter security posture on ARM64 / Raspberry Pi — read-only Docker socket, non-root UID 65532, no privileged mode, distroless image.
---

# Security

docker-exporter exposes Docker metrics with the smallest reasonable blast radius — which matters most on single-host Raspberry Pi and homelab setups without the Kubernetes-style network policies you'd otherwise lean on.

## Posture

- **Read-only socket.** The Docker socket is mounted `:ro` and the exporter only issues read calls (`list`, `inspect`, `stats`). It never creates, starts, stops, or execs into containers.
- **Non-root.** Runs as UID **65532** (the distroless `nonroot` user) — not root, inside or outside the container.
- **No privileged mode.** Unlike [cAdvisor](/compare/cadvisor), it needs no `--privileged` and no bind mounts of `/proc`, `/sys`, or the cgroup filesystem. The Docker socket is its only host dependency.
- **Minimal image.** A single static musl binary on `distroless/static` — no shell, no package manager, ~9 MB of attack surface.

## Is read-only socket access actually safe?

Even mounted read-only, the Docker socket is a **powerful** interface: read access exposes container configuration, and on most hosts a compromised exporter can use that access to escalate privileges — [Docker's own engine security docs](https://docs.docker.com/engine/security/#docker-daemon-attack-surface) treat access to the daemon socket as equivalent to root on the host. Treat docker-exporter like any component with socket access:

- Don't expose port `9713` to untrusted networks. Keep it on an internal monitoring network and let Prometheus reach it there.
- Pin the image to a specific version and update deliberately.
- If you need stronger isolation, run it behind a socket proxy that allowlists only the read endpoints it uses.

## Reporting a vulnerability

Follow the disclosure policy in [`SECURITY.md`](https://github.com/dlepaux/docker-exporter/blob/main/SECURITY.md) — do **not** open a public issue for security reports.
