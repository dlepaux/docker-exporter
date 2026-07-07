---
title: "Raspberry Pi 5 (ARM64 + cgroup v2): cAdvisor shows zero memory"
description: cAdvisor reports zero memory on Raspberry Pi 5 (ARM64 + cgroup v2) — one fixed with cgroup_enable=memory, one an unfixed cAdvisor bug (#3469). How to tell them apart and fix both.
head:
  - - script
    - type: application/ld+json
    - |-
      {"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"Why does cAdvisor show zero memory on a Raspberry Pi 5?","acceptedAnswer":{"@type":"Answer","text":"Two separate causes. First, Raspberry Pi OS ships with the memory cgroup controller disabled by default, so no tool can read memory until you add cgroup_enable=memory to /boot/firmware/cmdline.txt and reboot. Second, even after that, cAdvisor still mis-reports container_memory_working_set_bytes on ARM64 with cgroup v2 — an upstream bug (cAdvisor #3469) closed as 'not planned'."}},{"@type":"Question","name":"What is the correct flag to enable memory cgroups on Raspberry Pi OS?","acceptedAnswer":{"@type":"Answer","text":"Add cgroup_enable=memory to /boot/firmware/cmdline.txt and reboot. The older cgroup_memory=1 cgroup_enable=memory form from 2022-era guides is no longer needed; cgroup_enable=memory alone is correct on current Raspberry Pi OS."}},{"@type":"Question","name":"Will Raspberry Pi OS ever enable memory cgroups by default?","acceptedAnswer":{"@type":"Answer","text":"No. The Raspberry Pi kernel team has confirmed memory cgroups are intentionally available but disabled by default and must be opted into. This is a deliberate, permanent decision, so the requirement is evergreen."}},{"@type":"Question","name":"How do I fix cAdvisor's zero memory on ARM64?","acceptedAnswer":{"@type":"Answer","text":"If memory is still zero or wrong after enabling memory cgroups, it's cAdvisor bug #3469, not a config error. Use docker-exporter, which reads the Docker stats API directly and computes the working set correctly on cgroup v2 (usage minus inactive_file)."}}]}
---

# Why cAdvisor shows zero memory on Raspberry Pi 5 (ARM64 + cgroup v2)

**Short answer:** there are *two* separate causes, and most guides only cover the first. (1) Raspberry Pi OS ships with the memory cgroup controller **disabled by default**, so no tool can read container memory until you enable it. (2) Even after you enable it, **cAdvisor still mis-reports** `container_memory_working_set_bytes` on ARM64 + cgroup v2 — a known, unfixed upstream bug. Below: how to tell which one you're hitting, and how to fix both.

## Layer 1 — memory cgroups are disabled by default

Out of the box, Raspberry Pi OS boots with the kernel's memory cgroup controller disabled. `docker stats` and every exporter (cAdvisor included) then read **zero** memory, because the kernel isn't accounting it.

Fix it by adding one flag to `/boot/firmware/cmdline.txt` (all on the existing single line) and rebooting:

```
cgroup_enable=memory
```

::: warning Use the current flag
Older (2022-era) tutorials tell you to add `cgroup_memory=1 cgroup_enable=memory`. On current Raspberry Pi OS, **`cgroup_enable=memory` alone is correct** — and cgroup **v1** reporting was dropped entirely in Linux kernel 6.12+, so v2-only guidance now applies.
:::

Confirm it took effect after reboot:

```bash
docker info | grep -i memory        # should NOT say "WARNING: No memory limit support"
cat /sys/fs/cgroup/memory.current   # a real number, not missing
```

This requirement is **not going away**. The Raspberry Pi kernel maintainers confirm memory cgroups are *intentionally* disabled by default and must be opted in ([raspberrypi/linux#6980](https://github.com/raspberrypi/linux/issues/6980)) — reconfirmed on the 2026 Raspberry Pi OS "Trixie" release ([pi-gen#917](https://github.com/RPi-Distro/pi-gen/issues/917)). Enabling memory cgroups is a permanent step, not a temporary one.

## Layer 2 — cAdvisor still gets it wrong on ARM64 + cgroup v2

Here's the part almost no article covers. After you enable memory cgroups, `docker stats` shows correct numbers — but **cAdvisor still reports `container_memory_working_set_bytes` as zero** on Raspberry Pi 5 (ARM64 + cgroup v2).

This is an upstream bug, not your configuration: [cAdvisor #3469 — *"Memory Usage always zero"* (HW: Raspberry Pi 5)](https://github.com/google/cadvisor/issues/3469) was closed **"not planned"** on 2025-12-09. A commenter who had already applied the `cgroup_enable=memory` fix still saw incorrect memory readings. Broader ARM-side pain is tracked in [cAdvisor #2523](https://github.com/google/cadvisor/issues/2523). cAdvisor isn't sized or maintained for single-board ARM homelabs.

## How to tell which layer you're at

| Symptom | Cause | Fix |
| --- | --- | --- |
| `docker stats` **and** cAdvisor both show zero memory | Layer 1 — cgroups disabled | Add `cgroup_enable=memory`, reboot |
| `docker stats` correct, but cAdvisor shows zero / wrong | Layer 2 — cAdvisor ARM64 bug | Use docker-exporter |

## The fix: read the stats API directly

`docker-exporter` sidesteps Layer 2 entirely. It reads the Docker stats API directly and computes the working set using the kernel's own definition:

- **cgroup v2:** `usage − inactive_file`
- **cgroup v1:** `usage − cache`

It uses the same metric name as cAdvisor (`container_memory_working_set_bytes`), so your Grafana panels light up again — with real numbers instead of zero. It runs in ~7 MiB of RAM ([full footprint benchmark →](/why/benchmark)), read-only on the socket, non-root.

```bash
docker run -d \
  --name docker-exporter \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 --restart unless-stopped \
  ghcr.io/dlepaux/docker-exporter:latest
```

[Install guide →](/guide/installation) · [docker-exporter vs cAdvisor →](/compare/cadvisor)

## FAQ

### Why does cAdvisor show zero memory on a Raspberry Pi 5?
Two separate causes. First, Raspberry Pi OS ships with the memory cgroup controller **disabled by default**, so no tool can read container memory until you add `cgroup_enable=memory` to `/boot/firmware/cmdline.txt` and reboot. Second, even after that, cAdvisor still mis-reports `container_memory_working_set_bytes` on ARM64 with cgroup v2 — upstream bug [#3469](https://github.com/google/cadvisor/issues/3469), closed **"not planned"**.

### What is the correct flag to enable memory cgroups on Raspberry Pi OS?
Add `cgroup_enable=memory` to `/boot/firmware/cmdline.txt` (on the existing single line) and reboot. The older `cgroup_memory=1 cgroup_enable=memory` form from 2022-era guides is no longer needed — `cgroup_enable=memory` alone is correct on current Raspberry Pi OS.

### Will Raspberry Pi OS ever enable memory cgroups by default?
No. The Raspberry Pi kernel team has confirmed memory cgroups are *intentionally* available but disabled by default and must be opted into ([raspberrypi/linux#6980](https://github.com/raspberrypi/linux/issues/6980)). It's a deliberate, permanent decision, so the requirement is evergreen.

### How do I fix cAdvisor's zero memory on ARM64?
If memory is still zero or wrong after enabling memory cgroups, it's cAdvisor bug [#3469](https://github.com/google/cadvisor/issues/3469), not a config error. Use `docker-exporter`, which reads the Docker stats API directly and computes the working set correctly on cgroup v2 (`usage − inactive_file`).

### Is this the same as the "No memory limit support" warning?
That warning is Layer 1 (cgroups disabled). Fix it with `cgroup_enable=memory`. If memory is still wrong *after* the warning is gone, you're at Layer 2.

### Does enabling memory cgroups cost performance?
A small, fixed amount of kernel memory accounting overhead — negligible on a Pi 5, and required for any per-container memory metrics.

### Why not just patch cAdvisor?
The upstream issue is closed **"not planned,"** and cAdvisor's architecture walks the whole host/kernel/hardware topology by design — heavy for a single-board computer. A purpose-built exporter is the smaller, durable fix.
