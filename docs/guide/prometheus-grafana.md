---
title: Prometheus & Grafana
description: Scrape docker-exporter with Prometheus and Grafana on ARM64 / Raspberry Pi 5 — scrape config, alerting rules, and dashboards that finally read memory correctly.
---

# Prometheus & Grafana

**docker-exporter speaks cAdvisor's metric language**, so if you already run Prometheus and Grafana you can wire it in without rebuilding dashboards. On ARM64 / Raspberry Pi 5 it also makes memory panels stop reading zero — the result of an [unfixed cAdvisor bug (#3469)](/why/cadvisor-arm64-zero-memory) this exporter sidesteps.

## Prometheus scrape config

```yaml
scrape_configs:
  - job_name: docker
    static_configs:
      - targets: ["docker-exporter:9713"]
```

A 15 s scrape interval is fine for container metrics (Prometheus's own built-in default is actually 1 m). **Avoid going below 5 s** — each scrape issues a stats and an inspect call per container, so very short intervals mostly add load on the Docker daemon for little practical gain.

## Grafana dashboards and PromQL queries

docker-exporter uses [**cAdvisor-compatible metric names**](/compare/cadvisor) (`container_cpu_usage_seconds_total`, `container_memory_working_set_bytes`, `container_network_receive_bytes_total`, …). Most existing cAdvisor/Docker Grafana dashboards work against it with little or no query changes — name-keyed panels are drop-in, while panels keyed on cAdvisor-only labels (the per-core `cpu` label, or a cgroup-path `id`) may need minor edits. On ARM64 / Raspberry Pi 5 the memory panels finally show real numbers instead of zero.

A few queries to start from:

```promql
# Working set per container (the number cAdvisor zeroes on ARM64)
container_memory_working_set_bytes

# CPU cores used per container (rate of the cumulative counter)
rate(container_cpu_usage_seconds_total[5m])

# Network receive throughput
rate(container_network_receive_bytes_total[5m])

# Containers that stopped but shouldn't have (ignores one-shots)
container_state{restart_policy!="no"} == 0
```

::: info Official dashboard
A ready-made Grafana dashboard is on the roadmap and will be published on grafana.com. Until then, point any cAdvisor-based dashboard at your Prometheus data source — the metric names line up.
:::

## Alerting example

Two starter rules: alert when docker-exporter can't reach the Docker daemon, and when a container that shouldn't stop does.

```yaml
groups:
  - name: docker-exporter
    rules:
      - alert: DockerExporterDown
        expr: docker_exporter_up == 0
        for: 5m
        labels: { severity: warning }
        annotations:
          summary: "docker-exporter cannot reach the Docker daemon"

      - alert: ContainerStopped
        expr: container_state{restart_policy!="no"} == 0
        for: 10m
        labels: { severity: warning }
        annotations:
          summary: "Container {{ $labels.name }} is not running"
```

[Why cAdvisor shows zero memory →](/why/cadvisor-arm64-zero-memory) · [docker-exporter vs cAdvisor →](/compare/cadvisor) · [Metrics reference →](/guide/metrics)
