---
title: "Raspberry Pi 5: why container memory reads zero"
description: On a Raspberry Pi 5, container memory reads zero in docker stats, cAdvisor and every exporter until memory cgroups are enabled. One kernel flag, cgroup_enable=memory, fixes all of them.
head:
  - - script
    - type: application/ld+json
    - |-
      {"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"Why does container memory show zero on a Raspberry Pi 5?","acceptedAnswer":{"@type":"Answer","text":"The Raspberry Pi kernel boots with the memory cgroup controller disabled (cgroup_disable=memory is on its default command line), so docker stats, cAdvisor and every exporter read zero. Add cgroup_enable=memory to /boot/firmware/cmdline.txt and reboot."}},{"@type":"Question","name":"What is the correct flag to enable memory cgroups on Raspberry Pi OS?","acceptedAnswer":{"@type":"Answer","text":"Add cgroup_enable=memory to /boot/firmware/cmdline.txt, on the existing single line, and reboot. cgroup_memory=1 is not needed: the kernel applies the flags in order, and cmdline.txt comes after the default cgroup_disable=memory."}},{"@type":"Question","name":"Will Raspberry Pi OS ever enable memory cgroups by default?","acceptedAnswer":{"@type":"Answer","text":"No. The Raspberry Pi kernel team's stated preference is to keep the memory cgroup available but disabled by default, and a request to enable it by default in Raspberry Pi OS Lite was closed as not planned in April 2026."}},{"@type":"Question","name":"Does cAdvisor read memory correctly on a Raspberry Pi 5 once memory cgroups are enabled?","acceptedAnswer":{"@type":"Answer","text":"Yes. On a Raspberry Pi 5 with Docker 29.8.1, cAdvisor v0.55.1 and v0.60.6 reported the same working set as docker-exporter (tested 2026-09-28). cAdvisor v0.47.2 is too old for Docker 29 and reports no containers at all."}}]}
---

# Why container memory reads zero on a Raspberry Pi 5

**Short answer:** the Raspberry Pi kernel boots with its memory cgroup controller disabled, so nothing can read per-container memory: `docker stats`, cAdvisor and docker-exporter all show zero. Add `cgroup_enable=memory` to `/boot/firmware/cmdline.txt` and reboot. That one flag fixes every tool, cAdvisor included.

::: info Correction, 2026-09-28
This page used to say that cAdvisor keeps reporting zero memory on a Pi 5 even after memory cgroups are enabled, and that this was why docker-exporter exists. A test on a Raspberry Pi 5 showed otherwise: with memory cgroups on, cAdvisor reads memory correctly ([results below](#does-cadvisor-work-once-memory-cgroups-are-on)). The upstream report, [cAdvisor #3469](https://github.com/google/cadvisor/issues/3469), is this same boot setting: a commenter saw `docker stats` at zero as well, and a Pi 5 user got memory back after adding the flag. What docker-exporter changes is the footprint and the privileges, not the memory numbers.
:::

## The cause: the memory cgroup is off at boot

The Raspberry Pi kernel's default command line, inherited from the device tree, carries `cgroup_disable=memory`. The kernel then keeps no per-container memory accounting, and every tool that reads it reports zero.

This is deliberate. The Raspberry Pi kernel team's stated preference is to have the memory cgroup "available but disabled by default" ([raspberrypi/linux#6980](https://github.com/raspberrypi/linux/issues/6980#issuecomment-3149752155)), and a request to enable it by default in Raspberry Pi OS Lite was closed as not planned in April 2026 ([pi-gen#917](https://github.com/RPi-Distro/pi-gen/issues/917)). Expect to set it yourself on every Pi.

## The fix: one flag

Add this to `/boot/firmware/cmdline.txt`, on the existing single line, and reboot:

```
cgroup_enable=memory
```

`cgroup_memory=1`, which older guides also add, isn't needed. The kernel applies the flags in the order they appear, and `cmdline.txt` comes after the default `cgroup_disable=memory`, so yours wins: `/proc/cmdline` shows both, the disable first and your enable after it.

Raspberry Pi kernels from 6.12 are also built without cgroup v1 memory support, so `memory` no longer appears in `/proc/cgroups`. Check the cgroup v2 controller list instead:

```bash
cat /sys/fs/cgroup/cgroup.controllers   # lists "memory"
docker info | grep -i "memory limit"    # prints nothing: no "No memory limit support" warning
```

## Does cAdvisor work once memory cgroups are on?

Yes. Tested on 2026-09-28 on a Raspberry Pi 5 (Raspberry Pi kernel 6.18, Docker 29.8.1, cgroup v2, `cgroup_enable=memory`), with cAdvisor started from its README command plus `--docker_only=true`. Both exporters read `container_memory_working_set_bytes` within the same minute:

| Container | cAdvisor v0.60.6 | docker-exporter 1.6.0 |
| --- | --- | --- |
| vector | 28.7 MB | 28.7 MB |
| node-exporter | 19.0 MB | 19.5 MB |
| image-update-exporter | 6.2 MB | 5.2 MB |

The small differences come from when each read happened. cAdvisor v0.55.1 gave the same picture (vector 30.6 MB, node-exporter 20.1 MB).

If cAdvisor still shows nothing after you enable memory cgroups, check its version before its memory: v0.47.2, the version in #3469, can't identify containers on Docker 29 and publishes no per-container series at all (its log says `failed to identify the read-write layer ID`). Current releases are published as `ghcr.io/google/cadvisor`.

One more trap from the same thread: a dashboard that sums memory over every cAdvisor series counts the machine twice, because cAdvisor also exports the root cgroup. Filter to containers, for example `{name!=""}`.

## Where docker-exporter fits

It doesn't fix a zero: nothing can until the kernel accounts memory. Once it does, docker-exporter and cAdvisor report the same working set. What docker-exporter changes is the cost of getting it. On the same Pi 5 it used about a tenth of cAdvisor's CPU and less than half its memory, and it needs no privileged mode, only the Docker socket, read-only ([benchmark →](/why/benchmark)).

```bash
docker run -d \
  --name docker-exporter \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -p 9713:9713 --restart unless-stopped \
  ghcr.io/dlepaux/docker-exporter:latest
```

[Install guide →](/guide/installation) · [docker-exporter vs cAdvisor →](/compare/cadvisor)

## FAQ

### Why does container memory show zero on a Raspberry Pi 5?
The Raspberry Pi kernel boots with the memory cgroup controller disabled (`cgroup_disable=memory` is on its default command line), so `docker stats`, cAdvisor and every exporter read zero. Add `cgroup_enable=memory` to `/boot/firmware/cmdline.txt` and reboot.

### What is the correct flag to enable memory cgroups on Raspberry Pi OS?
`cgroup_enable=memory` in `/boot/firmware/cmdline.txt`, on the existing single line, then reboot. `cgroup_memory=1` is not needed: the kernel applies the flags in order, and `cmdline.txt` comes after the default `cgroup_disable=memory`.

### Will Raspberry Pi OS ever enable memory cgroups by default?
No. The Raspberry Pi kernel team's stated preference is to keep the memory cgroup available but disabled by default ([raspberrypi/linux#6980](https://github.com/raspberrypi/linux/issues/6980#issuecomment-3149752155)), and a request to enable it by default in Raspberry Pi OS Lite was closed as not planned in April 2026 ([pi-gen#917](https://github.com/RPi-Distro/pi-gen/issues/917)).

### Does cAdvisor read memory correctly on a Raspberry Pi 5 once memory cgroups are enabled?
Yes. On a Raspberry Pi 5 with Docker 29.8.1, cAdvisor v0.55.1 and v0.60.6 reported the same working set as docker-exporter (tested 2026-09-28, [results above](#does-cadvisor-work-once-memory-cgroups-are-on)). cAdvisor v0.47.2 is too old for Docker 29 and reports no containers at all.

### Is this the same as the "No memory limit support" warning?
Yes. Docker prints that warning when the memory cgroup is unavailable, and the same flag makes it go away.

### Does enabling memory cgroups cost anything?
The kernel then accounts memory for every cgroup. Before changing the default, Raspberry Pi's maintainers asked for evidence that this costs nothing for users who don't need it ([pi-gen#917](https://github.com/RPi-Distro/pi-gen/issues/917)). Any per-container memory metric does need it.
