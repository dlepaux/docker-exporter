use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bollard::Docker;
use bollard::models::{ContainerNetworkStats, ContainerStatsResponse};
use bollard::query_parameters::{ListContainersOptionsBuilder, StatsOptionsBuilder};
use futures::stream::{self, StreamExt};

use crate::config::ExcludeMatcher;

/// Per-container Docker API timeout (stats and inspect each get their own).
const CONTAINER_QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum number of containers queried concurrently.
///
/// Every in-flight container holds up to two unix-socket connections (stats +
/// inspect run concurrently), and hyper opens a fresh fd per connection — a
/// pooled HTTP/1.1 connection serves one request at a time, and the pool caps
/// only *idle* connections, never concurrent ones. An unbounded fan-out over N
/// containers therefore attempts ~2N simultaneous `connect()` calls and blows
/// past `RLIMIT_NOFILE` (soft 1024 under Docker's default). Past that ceiling
/// every connect fails with EMFILE, which hyper reports only as the opaque
/// "client error (Connect)" — the errno lives in a Display-invisible `source`.
///
/// Bounding the fan-out caps peak fds at 2×this regardless of N, which is what
/// makes the exporter safe on hosts with thousands of containers. 64 keeps the
/// scrape a few short waves on a large host while staying an order of magnitude
/// under the default limit.
const MAX_CONCURRENT_CONTAINER_QUERIES: usize = 64;

/// Each in-flight container may hold two sockets at once (stats + inspect), and
/// Docker's default soft `RLIMIT_NOFILE` is 1024. Enforced at compile time so
/// raising the bound cannot silently re-create the 2026-07-09 fd exhaustion.
const _: () = assert!(
    MAX_CONCURRENT_CONTAINER_QUERIES * 2 < 1024,
    "concurrency bound must leave fd headroom under the default RLIMIT_NOFILE"
);

/// Per-scrape cap on individual container WARN lines, applied per failure kind.
///
/// The `*_failures_total` counters are the alertable signal; these log lines are
/// forensic samples. Uncapped, a daemon that fails every container turns one
/// scrape into 2N lines — at N=2080 on a 15s interval that measured ~19.7M
/// lines/day. A bounded sample plus one aggregate line preserves the forensics
/// without the flood.
const MAX_WARN_LINES_PER_KIND: usize = 10;

/// Collected metrics for a single container.
#[derive(Debug, Clone)]
pub struct ContainerMetrics {
    pub name: String,
    pub id: String,
    pub image: String,
    pub state: String,
    pub health: String,
    pub restart_policy: String,
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
    pub stats_failures_total: u64,
}

/// Collect metrics for all running containers.
///
/// This is called on each Prometheus scrape — no background polling.
/// Stats are fetched concurrently for all containers with a per-container timeout.
pub async fn collect(
    docker: &Docker,
    exclude: &ExcludeMatcher,
    inspect_failures: &AtomicU64,
    stats_failures: &AtomicU64,
) -> ScrapeResult {
    let start = Instant::now();

    let containers = match list_and_collect(docker, exclude, inspect_failures, stats_failures).await
    {
        Ok(containers) => containers,
        Err(err) => {
            tracing::error!(%err, "failed to collect container metrics");
            return ScrapeResult {
                containers: vec![],
                scrape_duration_seconds: start.elapsed().as_secs_f64(),
                docker_up: false,
                inspect_failures_total: inspect_failures.load(Ordering::Relaxed),
                stats_failures_total: stats_failures.load(Ordering::Relaxed),
            };
        }
    };

    ScrapeResult {
        containers,
        scrape_duration_seconds: start.elapsed().as_secs_f64(),
        docker_up: true,
        inspect_failures_total: inspect_failures.load(Ordering::Relaxed),
        stats_failures_total: stats_failures.load(Ordering::Relaxed),
    }
}

/// Drive `futures` to completion, keeping at most `max_concurrency` in flight.
///
/// This is the bound that keeps peak open file descriptors proportional to
/// `max_concurrency` rather than to the container count. See
/// [`MAX_CONCURRENT_CONTAINER_QUERIES`] for why an unbounded fan-out is a bug
/// and not merely an inefficiency.
///
/// Results come back in completion order, not input order; every caller pairs
/// each result with its own container, so order carries no meaning.
async fn buffer_bounded<I>(futures: I, max_concurrency: usize) -> Vec<<I::Item as Future>::Output>
where
    I: IntoIterator,
    I::Item: Future,
{
    stream::iter(futures)
        // `max(1)` because `buffer_unordered(0)` never polls anything and hangs.
        .buffer_unordered(max_concurrency.max(1))
        .collect()
        .await
}

/// Whether the Docker stats endpoint can return anything for a container in
/// this state.
///
/// Stats are read from live cgroup counters, so only a container with a running
/// (or frozen-but-live) task has them. For every other state Docker closes the
/// stats stream empty, which [`fetch_stats`] surfaces as a synthetic 404.
fn should_fetch_stats(state: &str) -> bool {
    matches!(state, "running" | "paused")
}

/// A per-scrape allowance of log lines for one failure kind.
///
/// Bounds an otherwise unbounded resource: without it the log volume of a
/// degraded scrape scales with the container count.
struct WarnBudget {
    remaining: usize,
    emitted: u64,
}

impl WarnBudget {
    fn new(cap: usize) -> Self {
        Self {
            remaining: cap,
            emitted: 0,
        }
    }

    /// Claim one line. Returns `false` once the budget for this scrape is spent.
    fn take(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.emitted += 1;
        true
    }
}

async fn list_and_collect(
    docker: &Docker,
    exclude: &ExcludeMatcher,
    inspect_failures: &AtomicU64,
    stats_failures: &AtomicU64,
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

    // Build one query per container (excluding filtered ones). Futures are inert
    // until polled, so this allocates cheaply — `buffer_bounded` below decides how
    // many actually run, and therefore how many sockets are ever open at once.
    let queries: Vec<_> = container_list
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
            !exclude.is_match(name)
        })
        .map(|container| {
            let id = container.id.clone().unwrap_or_default();
            // The list summary already carries the state, so we know before spending a
            // connection whether stats can exist at all.
            let state = container
                .state
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            let docker = docker.clone();
            async move {
                // `None` = deliberately not fetched. It yields the same zeroed metrics
                // as a failed fetch, so skipping a doomed call for a non-running
                // container changes no output — it only saves a socket and a WARN line.
                let stats_fut = async {
                    if should_fetch_stats(&state) {
                        Some(
                            tokio::time::timeout(
                                CONTAINER_QUERY_TIMEOUT,
                                fetch_stats(&docker, &id),
                            )
                            .await,
                        )
                    } else {
                        None
                    }
                };
                // structured health only exists in the inspect response, not the list
                // summary — so add a concurrent inspect call (own 5s timeout).
                let inspect_fut = tokio::time::timeout(
                    CONTAINER_QUERY_TIMEOUT,
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

    let results = buffer_bounded(queries, MAX_CONCURRENT_CONTAINER_QUERIES).await;

    let mut metrics = Vec::with_capacity(results.len());
    let mut stats_warns = WarnBudget::new(MAX_WARN_LINES_PER_KIND);
    let mut inspect_warns = WarnBudget::new(MAX_WARN_LINES_PER_KIND);
    let mut stats_failed: u64 = 0;
    let mut inspect_failed: u64 = 0;

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
            // Not running: stats were never requested (see `should_fetch_stats`).
            None => None,
            Some(Ok(Ok(s))) => Some(s),
            Some(Ok(Err(err))) => {
                stats_failed += 1;
                stats_failures.fetch_add(1, Ordering::Relaxed);
                if stats_warns.take() {
                    tracing::warn!(container = %name, %err, "failed to fetch stats");
                }
                None
            }
            Some(Err(_)) => {
                stats_failed += 1;
                stats_failures.fetch_add(1, Ordering::Relaxed);
                if stats_warns.take() {
                    tracing::warn!(container = %name, "stats fetch timed out (5s)");
                }
                None
            }
        };

        // Health from inspect. HealthStatusEnum impls Display (like ContainerSummaryStateEnum
        // used for `state` above) → .to_string() yields the API value; normalize it.
        // Health + restart policy both come from the SAME inspect response (two
        // independent fields → partial moves are fine). restart_policy lets the
        // alerting layer exclude intentional one-shots (restart:no) by label
        // instead of a hand-curated name blacklist. On inspect FAILURE → "unknown"
        // (never "no"): an unknown policy must still alert (fail-safe), never
        // silently exempt a real crashed service.
        let (health, restart_policy) = match inspect_result {
            Ok(Ok(inspect)) => {
                let raw = inspect
                    .state
                    .and_then(|s| s.health)
                    .and_then(|h| h.status)
                    .map(|st| st.to_string());
                let policy = inspect
                    .host_config
                    .and_then(|h| h.restart_policy)
                    .and_then(|r| r.name)
                    .map(|n| n.to_string());
                (normalize_health(raw), normalize_restart_policy(policy))
            }
            Ok(Err(err)) => {
                inspect_failed += 1;
                inspect_failures.fetch_add(1, Ordering::Relaxed);
                if inspect_warns.take() {
                    tracing::warn!(container = %name, %err, reason = "error", "inspect failed");
                }
                ("none".to_owned(), "unknown".to_owned())
            }
            Err(_) => {
                inspect_failed += 1;
                inspect_failures.fetch_add(1, Ordering::Relaxed);
                if inspect_warns.take() {
                    tracing::warn!(container = %name, reason = "timeout", "inspect timed out (5s)");
                }
                ("none".to_owned(), "unknown".to_owned())
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
            restart_policy,
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

    // Whatever the per-container budget swallowed still has to be visible once.
    let suppressed =
        (stats_failed - stats_warns.emitted) + (inspect_failed - inspect_warns.emitted);
    if suppressed > 0 {
        tracing::warn!(
            containers = metrics.len(),
            stats_failures = stats_failed,
            inspect_failures = inspect_failed,
            suppressed,
            "container metric collection degraded; per-container warnings suppressed for this scrape"
        );
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
// Docker reports a `--restart no` / unset policy as an empty name (older daemons)
// or "no" — both mean a one-shot / batch container. Fold them to "no" so the
// alerting layer can exclude one-shots by label. Any explicit policy (always,
// on-failure, unless-stopped) passes through verbatim. Inspect FAILURE is handled
// by the caller (→ "unknown"), never routed here, so a crash never reads as "no".
fn normalize_restart_policy(name: Option<String>) -> String {
    match name {
        Some(s) if !s.trim().is_empty() => s,
        _ => "no".to_owned(),
    }
}

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
    use super::{
        WarnBudget, buffer_bounded, normalize_health, normalize_restart_policy, should_fetch_stats,
    };
    use crate::config::ExcludeMatcher;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The load-bearing regression test for the 2026-07-09 log flood: an
    /// unbounded fan-out opened ~2N sockets at once, hit the 1024 fd ceiling,
    /// and turned every scrape into thousands of EMFILE WARN lines. Peak
    /// in-flight work must depend on the bound, never on the input size.
    #[tokio::test]
    async fn buffer_bounded_never_exceeds_max_concurrency() {
        let inflight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let limit = 8;
        let items = 500;

        let queries = (0..items).map(|i| {
            let inflight = Arc::clone(&inflight);
            let peak = Arc::clone(&peak);
            async move {
                let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                // Yield so the executor can poll the other buffered futures —
                // without this every future completes on first poll and nothing
                // is ever concurrent, which would make the assertion vacuous.
                tokio::task::yield_now().await;
                inflight.fetch_sub(1, Ordering::SeqCst);
                i
            }
        });

        let out = buffer_bounded(queries, limit).await;

        assert_eq!(out.len(), items, "every item must still be processed");
        let peak = peak.load(Ordering::SeqCst);
        assert!(
            peak <= limit,
            "peak concurrency {peak} exceeded the bound {limit}"
        );
        assert!(
            peak > 1,
            "work ran serially (peak {peak}) — the test would pass even unbounded"
        );
    }

    #[tokio::test]
    async fn buffer_bounded_treats_zero_as_serial_not_deadlock() {
        // buffer_unordered(0) would hang forever; the bound is clamped to >= 1.
        let mut out = buffer_bounded((0..3).map(|i| async move { i * 2 }), 0).await;
        out.sort_unstable();
        assert_eq!(out, vec![0, 2, 4]);
    }

    #[test]
    fn stats_are_only_fetched_for_live_containers() {
        // Live: cgroup counters exist.
        assert!(should_fetch_stats("running"));
        assert!(should_fetch_stats("paused"));
        // Not live: Docker closes the stats stream empty, which `fetch_stats`
        // turns into a synthetic 404. Asking costs a socket and a WARN line.
        assert!(!should_fetch_stats("exited"));
        assert!(!should_fetch_stats("created"));
        assert!(!should_fetch_stats("restarting"));
        assert!(!should_fetch_stats("removing"));
        assert!(!should_fetch_stats("dead"));
        // Unknown/absent state must not spend a doomed call either.
        assert!(!should_fetch_stats(""));
        assert!(!should_fetch_stats("Running"));
    }

    #[test]
    fn warn_budget_caps_lines_and_counts_what_it_emitted() {
        let mut budget = WarnBudget::new(2);
        assert!(budget.take());
        assert!(budget.take());
        assert!(!budget.take(), "budget must refuse once spent");
        assert!(!budget.take());
        assert_eq!(budget.emitted, 2, "only granted lines count as emitted");
    }

    #[test]
    fn warn_budget_of_zero_emits_nothing() {
        let mut budget = WarnBudget::new(0);
        assert!(!budget.take());
        assert_eq!(budget.emitted, 0);
    }

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
    fn restart_policy_empty_or_none_folds_to_no() {
        // Docker's unset / `--restart no` policy == a one-shot/batch container.
        assert_eq!(normalize_restart_policy(None), "no");
        assert_eq!(normalize_restart_policy(Some("".into())), "no");
        assert_eq!(normalize_restart_policy(Some("   ".into())), "no");
        // Explicit policies pass through, so a long-lived service stays alertable.
        assert_eq!(normalize_restart_policy(Some("always".into())), "always");
        assert_eq!(
            normalize_restart_policy(Some("on-failure".into())),
            "on-failure"
        );
        assert_eq!(
            normalize_restart_policy(Some("unless-stopped".into())),
            "unless-stopped"
        );
        assert_eq!(normalize_restart_policy(Some("no".into())), "no");
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

    // Exercise the real filter predicate used in `list_and_collect`
    // (`exclude.is_empty() || !exclude.is_match(name)`) through the
    // production `ExcludeMatcher`, covering both exact and glob entries.
    #[test]
    fn exclusion_filter_logic() {
        let exclude = ExcludeMatcher::parse("cadvisor,prometheus,debug-*").unwrap();
        let names = vec![
            "nginx",
            "cadvisor",
            "grafana",
            "prometheus",
            "debug-sidecar",
        ];

        let kept: Vec<_> = names
            .into_iter()
            .filter(|name| exclude.is_empty() || !exclude.is_match(name))
            .collect();

        assert_eq!(kept, vec!["nginx", "grafana"]);
    }

    #[test]
    fn exclusion_filter_empty_excludes_nothing() {
        let exclude = ExcludeMatcher::parse("").unwrap();
        let names = vec!["nginx", "cadvisor"];

        let kept: Vec<_> = names
            .into_iter()
            .filter(|name| exclude.is_empty() || !exclude.is_match(name))
            .collect();

        assert_eq!(kept, vec!["nginx", "cadvisor"]);
    }
}
