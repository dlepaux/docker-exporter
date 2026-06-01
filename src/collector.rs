use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bollard::Docker;
use bollard::models::{ContainerNetworkStats, ContainerStatsResponse};
use bollard::query_parameters::{ListContainersOptionsBuilder, StatsOptionsBuilder};
use futures::future::join_all;
use futures::stream::StreamExt;

/// Collected metrics for a single container.
#[derive(Debug, Clone)]
pub struct ContainerMetrics {
    pub name: String,
    pub id: String,
    pub image: String,
    pub state: String,
    pub health: String,
    pub cpu_usage_seconds: f64,
    pub memory_usage_bytes: f64,
    pub memory_working_set_bytes: f64,
    pub memory_cache_bytes: f64,
    pub memory_limit_bytes: f64,
    pub network: Vec<NetworkMetrics>,
    pub block_io_read_bytes: f64,
    pub block_io_write_bytes: f64,
    pub started_at: f64,
    pub last_seen: f64,
}

#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub interface: String,
    pub rx_bytes: f64,
    pub tx_bytes: f64,
    pub rx_packets: f64,
    pub tx_packets: f64,
    pub rx_errors: f64,
    pub tx_errors: f64,
    pub rx_dropped: f64,
    pub tx_dropped: f64,
}

/// Result of a full scrape cycle.
pub struct ScrapeResult {
    pub containers: Vec<ContainerMetrics>,
    pub scrape_duration_seconds: f64,
    pub docker_up: bool,
    pub inspect_failures_total: u64,
}

/// Collect metrics for all running containers.
///
/// This is called on each Prometheus scrape — no background polling.
/// Stats are fetched concurrently for all containers with a per-container timeout.
pub async fn collect(
    docker: &Docker,
    exclude: &[String],
    inspect_failures: &AtomicU64,
) -> ScrapeResult {
    let start = Instant::now();

    let containers = match list_and_collect(docker, exclude, inspect_failures).await {
        Ok(containers) => containers,
        Err(err) => {
            tracing::error!(%err, "failed to collect container metrics");
            return ScrapeResult {
                containers: vec![],
                scrape_duration_seconds: start.elapsed().as_secs_f64(),
                docker_up: false,
                inspect_failures_total: inspect_failures.load(Ordering::Relaxed),
            };
        }
    };

    ScrapeResult {
        containers,
        scrape_duration_seconds: start.elapsed().as_secs_f64(),
        docker_up: true,
        inspect_failures_total: inspect_failures.load(Ordering::Relaxed),
    }
}

async fn list_and_collect(
    docker: &Docker,
    exclude: &[String],
    inspect_failures: &AtomicU64,
) -> Result<Vec<ContainerMetrics>, bollard::errors::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    // List all containers (including stopped) so we can report state
    let container_list = docker
        .list_containers(Some(
            ListContainersOptionsBuilder::default().all(true).build(),
        ))
        .await?;

    // Fetch stats concurrently for all containers (excluding filtered ones)
    let futures: Vec<_> = container_list
        .iter()
        .filter(|container| {
            if exclude.is_empty() {
                return true;
            }
            let name = container
                .names
                .as_ref()
                .and_then(|names| names.first())
                .map(|n| n.strip_prefix('/').unwrap_or(n))
                .unwrap_or("");
            !exclude.iter().any(|e| e == name)
        })
        .map(|container| {
            let id = container.id.clone().unwrap_or_default();
            let docker = docker.clone();
            async move {
                let stats_fut = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    fetch_stats(&docker, &id),
                );
                // structured health only exists in the inspect response, not the list
                // summary — so add a concurrent inspect call (own 5s timeout).
                let inspect_fut = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    docker.inspect_container(
                        &id,
                        None::<bollard::query_parameters::InspectContainerOptions>,
                    ),
                );
                let (stats_result, inspect_result) = tokio::join!(stats_fut, inspect_fut);
                (container, stats_result, inspect_result)
            }
        })
        .collect();

    let results = join_all(futures).await;

    let mut metrics = Vec::with_capacity(results.len());

    for (container, stats_result, inspect_result) in results {
        let id = container.id.clone().unwrap_or_default();
        let name = container
            .names
            .as_ref()
            .and_then(|names| names.first())
            .map(|n| n.strip_prefix('/').unwrap_or(n))
            .unwrap_or("unknown")
            .to_owned();
        let image = container.image.clone().unwrap_or_default();
        // ContainerSummaryStateEnum implements Display
        let state = container
            .state
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let created = container.created.unwrap_or(0) as f64;

        // Container names are validated by Docker against [a-zA-Z0-9][a-zA-Z0-9_.-]+,
        // so they cannot contain newlines, quotes, or path traversal characters.
        // If this collector is ever extended to emit container *labels* (which DO
        // accept arbitrary UTF-8), the input must be sanitized before reaching
        // `tracing` or Prometheus output.
        let stats = match stats_result {
            Ok(Ok(s)) => Some(s),
            Ok(Err(err)) => {
                tracing::warn!(container = %name, %err, "failed to fetch stats");
                None
            }
            Err(_) => {
                tracing::warn!(container = %name, "stats fetch timed out (5s)");
                None
            }
        };

        // Health from inspect. HealthStatusEnum impls Display (like ContainerSummaryStateEnum
        // used for `state` above) → .to_string() yields the API value; normalize it.
        let health = match inspect_result {
            Ok(Ok(inspect)) => {
                let raw = inspect
                    .state
                    .and_then(|s| s.health)
                    .and_then(|h| h.status)
                    .map(|st| st.to_string());
                normalize_health(raw)
            }
            Ok(Err(err)) => {
                tracing::warn!(container = %name, %err, reason = "error", "inspect failed");
                inspect_failures.fetch_add(1, Ordering::Relaxed);
                "none".to_owned()
            }
            Err(_) => {
                tracing::warn!(container = %name, reason = "timeout", "inspect timed out (5s)");
                inspect_failures.fetch_add(1, Ordering::Relaxed);
                "none".to_owned()
            }
        };

        let (cpu, mem_usage, mem_working_set, mem_cache, mem_limit, network, bio_read, bio_write) =
            if let Some(ref stats) = stats {
                (
                    extract_cpu_seconds(stats),
                    extract_memory_usage(stats),
                    extract_memory_working_set(stats),
                    extract_memory_cache(stats),
                    extract_memory_limit(stats),
                    extract_network(stats),
                    extract_blkio_read(stats),
                    extract_blkio_write(stats),
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, vec![], 0.0, 0.0)
            };

        metrics.push(ContainerMetrics {
            name,
            id,
            image,
            state,
            health,
            cpu_usage_seconds: cpu,
            memory_usage_bytes: mem_usage,
            memory_working_set_bytes: mem_working_set,
            memory_cache_bytes: mem_cache,
            memory_limit_bytes: mem_limit,
            network,
            block_io_read_bytes: bio_read,
            block_io_write_bytes: bio_write,
            started_at: created,
            last_seen: now,
        });
    }

    Ok(metrics)
}

async fn fetch_stats(
    docker: &Docker,
    container_id: &str,
) -> Result<ContainerStatsResponse, bollard::errors::Error> {
    let options = StatsOptionsBuilder::default().stream(false).build();

    docker
        .stats(container_id, Some(options))
        .take(1)
        .next()
        .await
        .unwrap_or_else(|| {
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404,
                message: "no stats returned".into(),
            })
        })
}

/// Normalize Docker's health status to one of: healthy | unhealthy | starting | none.
/// Docker's inspect reports "healthy"/"unhealthy"/"starting"/"none"/"" (and absent when
/// no healthcheck is configured). Anything not explicitly healthy/unhealthy/starting folds
/// to "none" — a missing or unknown signal must never read as unhealthy.
fn normalize_health(raw: Option<String>) -> String {
    let s = raw.unwrap_or_default().trim().to_ascii_lowercase();
    match s.as_str() {
        "healthy" => "healthy",
        "unhealthy" => "unhealthy",
        "starting" => "starting",
        _ => "none",
    }
    .to_owned()
}

/// CPU usage in cumulative seconds (converted from nanoseconds).
fn extract_cpu_seconds(stats: &ContainerStatsResponse) -> f64 {
    let total_ns = stats
        .cpu_stats
        .as_ref()
        .and_then(|c| c.cpu_usage.as_ref())
        .and_then(|u| u.total_usage)
        .unwrap_or(0);

    total_ns as f64 / 1_000_000_000.0
}

/// Raw memory usage in bytes.
fn extract_memory_usage(stats: &ContainerStatsResponse) -> f64 {
    stats
        .memory_stats
        .as_ref()
        .and_then(|m| m.usage)
        .unwrap_or(0) as f64
}

/// Memory working set: usage minus cache.
/// cgroup v2: usage - inactive_file
/// cgroup v1: usage - cache
fn extract_memory_working_set(stats: &ContainerStatsResponse) -> f64 {
    let usage = stats
        .memory_stats
        .as_ref()
        .and_then(|m| m.usage)
        .unwrap_or(0) as f64;

    let cache = extract_memory_cache(stats);

    (usage - cache).max(0.0)
}

/// Memory cache bytes.
/// cgroup v2 uses `inactive_file`, cgroup v1 uses `cache`.
fn extract_memory_cache(stats: &ContainerStatsResponse) -> f64 {
    let mem_stats: &HashMap<String, u64> =
        match stats.memory_stats.as_ref().and_then(|m| m.stats.as_ref()) {
            Some(s) => s,
            None => return 0.0,
        };

    // Try cgroup v2 field first, then v1.
    if let Some(&val) = mem_stats.get("inactive_file") {
        return val as f64;
    }
    if let Some(&val) = mem_stats.get("cache") {
        return val as f64;
    }

    0.0
}

/// Memory limit in bytes.
fn extract_memory_limit(stats: &ContainerStatsResponse) -> f64 {
    stats
        .memory_stats
        .as_ref()
        .and_then(|m| m.limit)
        .unwrap_or(0) as f64
}

/// Network metrics per interface.
fn extract_network(stats: &ContainerStatsResponse) -> Vec<NetworkMetrics> {
    let networks: &HashMap<String, ContainerNetworkStats> = match stats.networks.as_ref() {
        Some(n) => n,
        None => return vec![],
    };

    networks
        .iter()
        .map(|(interface, net)| NetworkMetrics {
            interface: interface.clone(),
            rx_bytes: net.rx_bytes.unwrap_or(0) as f64,
            tx_bytes: net.tx_bytes.unwrap_or(0) as f64,
            rx_packets: net.rx_packets.unwrap_or(0) as f64,
            tx_packets: net.tx_packets.unwrap_or(0) as f64,
            rx_errors: net.rx_errors.unwrap_or(0) as f64,
            tx_errors: net.tx_errors.unwrap_or(0) as f64,
            rx_dropped: net.rx_dropped.unwrap_or(0) as f64,
            tx_dropped: net.tx_dropped.unwrap_or(0) as f64,
        })
        .collect()
}

/// Block I/O read bytes from blkio_stats.
fn extract_blkio_read(stats: &ContainerStatsResponse) -> f64 {
    extract_blkio(stats, "read")
}

/// Block I/O write bytes from blkio_stats.
fn extract_blkio_write(stats: &ContainerStatsResponse) -> f64 {
    extract_blkio(stats, "write")
}

fn extract_blkio(stats: &ContainerStatsResponse, op: &str) -> f64 {
    let entries = match stats
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_service_bytes_recursive.as_ref())
    {
        Some(e) => e,
        None => return 0.0,
    };

    entries
        .iter()
        .filter(|e| e.op.as_deref().is_some_and(|o| o.eq_ignore_ascii_case(op)))
        .map(|e| e.value.unwrap_or(0) as f64)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::normalize_health;

    #[test]
    fn normalize_health_maps_known_states() {
        assert_eq!(normalize_health(Some("healthy".into())), "healthy");
        assert_eq!(normalize_health(Some("unhealthy".into())), "unhealthy");
        assert_eq!(normalize_health(Some("starting".into())), "starting");
    }

    #[test]
    fn normalize_health_folds_unknown_and_absent_to_none() {
        assert_eq!(normalize_health(None), "none");
        assert_eq!(normalize_health(Some("".into())), "none");
        assert_eq!(normalize_health(Some("created".into())), "none");
        assert_eq!(normalize_health(Some("HEALTHY".into())), "healthy"); // case-insensitive
        assert_eq!(normalize_health(Some("  starting ".into())), "starting"); // trimmed
    }

    #[test]
    fn cpu_nanoseconds_to_seconds() {
        let seconds = 1_500_000_000_f64 / 1_000_000_000.0;
        assert!((seconds - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn working_set_never_negative() {
        let usage: f64 = 100.0;
        let cache: f64 = 200.0;
        let working_set = (usage - cache).max(0.0);
        assert_eq!(working_set, 0.0);
    }

    #[test]
    fn container_name_strip_prefix() {
        let name = "/my-container";
        let stripped = name.strip_prefix('/').unwrap_or(name);
        assert_eq!(stripped, "my-container");

        let name = "no-prefix";
        let stripped = name.strip_prefix('/').unwrap_or(name);
        assert_eq!(stripped, "no-prefix");
    }

    #[test]
    fn exclusion_filter_logic() {
        let exclude = ["cadvisor".to_owned(), "prometheus".to_owned()];
        let names = vec!["nginx", "cadvisor", "grafana", "prometheus", "my-app"];

        let kept: Vec<_> = names
            .into_iter()
            .filter(|name| !exclude.iter().any(|e| e == name))
            .collect();

        assert_eq!(kept, vec!["nginx", "grafana", "my-app"]);
    }

    #[test]
    fn exclusion_filter_empty_excludes_nothing() {
        let exclude: Vec<String> = vec![];
        let names = vec!["nginx", "cadvisor"];

        let kept: Vec<_> = names
            .into_iter()
            .filter(|name| exclude.is_empty() || !exclude.iter().any(|e| e == name))
            .collect();

        assert_eq!(kept, vec!["nginx", "cadvisor"]);
    }
}
