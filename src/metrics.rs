use prometheus::TextEncoder;
use prometheus::proto::{Counter, Gauge, LabelPair, Metric, MetricFamily, MetricType};

use crate::collector::{ContainerMetrics, ScrapeResult};

/// Encode a scrape result into Prometheus text exposition format.
///
/// Builds MetricFamily proto structs directly (no Registry) so we can:
/// - Emit counter-typed metrics with absolute values
/// - Avoid stale metric cleanup (rebuilt from scratch each scrape)
/// - Have zero shared mutable state
pub fn encode(result: &ScrapeResult) -> String {
    let families = build_metric_families(result);
    let encoder = TextEncoder::new();
    encoder.encode_to_string(&families).unwrap_or_default()
}

fn build_metric_families(result: &ScrapeResult) -> Vec<MetricFamily> {
    let mut families = Vec::new();

    // Exporter meta metrics
    families.push(gauge_family(
        "docker_exporter_up",
        "Whether the Docker daemon is reachable (1 = up, 0 = down)",
        vec![gauge_metric(&[], if result.docker_up { 1.0 } else { 0.0 })],
    ));

    families.push(gauge_family(
        "docker_exporter_scrape_duration_seconds",
        "Duration of the last scrape in seconds",
        vec![gauge_metric(&[], result.scrape_duration_seconds)],
    ));

    families.push(counter_family(
        "docker_exporter_inspect_failures_total",
        "Total container inspect failures since exporter start",
        vec![counter_metric(&[], result.inspect_failures_total as f64)],
    ));

    if result.containers.is_empty() {
        return families;
    }

    // CPU
    let mut cpu_metrics = Vec::new();
    // Memory
    let mut mem_usage_metrics = Vec::new();
    let mut mem_working_set_metrics = Vec::new();
    let mut mem_cache_metrics = Vec::new();
    let mut mem_limit_metrics = Vec::new();
    // Network
    let mut net_rx_metrics = Vec::new();
    let mut net_tx_metrics = Vec::new();
    // Block I/O
    let mut blkio_metrics = Vec::new();
    // State & lifecycle
    let mut state_metrics = Vec::new();
    let mut health_metrics = Vec::new();
    let mut start_time_metrics = Vec::new();
    let mut last_seen_metrics = Vec::new();

    for c in &result.containers {
        let base_labels = base_labels(c);

        // CPU — counter (cumulative seconds)
        cpu_metrics.push(counter_metric(&base_labels, c.cpu_usage_seconds));

        // Memory — gauges
        mem_usage_metrics.push(gauge_metric(&base_labels, c.memory_usage_bytes));
        mem_working_set_metrics.push(gauge_metric(&base_labels, c.memory_working_set_bytes));
        mem_cache_metrics.push(gauge_metric(&base_labels, c.memory_cache_bytes));
        mem_limit_metrics.push(gauge_metric(&base_labels, c.memory_limit_bytes));

        // Network — counters per interface
        for net in &c.network {
            let mut net_labels = base_labels.clone();
            net_labels.push(label("interface", &net.interface));

            net_rx_metrics.push(counter_metric(&net_labels, net.rx_bytes));
            net_tx_metrics.push(counter_metric(&net_labels, net.tx_bytes));
        }

        // Block I/O — counters per operation
        if c.block_io_read_bytes > 0.0 || c.block_io_write_bytes > 0.0 {
            let mut read_labels = base_labels.clone();
            read_labels.push(label("operation", "read"));
            blkio_metrics.push(counter_metric(&read_labels, c.block_io_read_bytes));

            let mut write_labels = base_labels.clone();
            write_labels.push(label("operation", "write"));
            blkio_metrics.push(counter_metric(&write_labels, c.block_io_write_bytes));
        }

        // State — gauge (1 = running, 0 = other)
        let state_value = if c.state == "running" { 1.0 } else { 0.0 };
        let mut state_labels = base_labels.clone();
        state_labels.push(label("state", &c.state));
        // restart_policy lives ONLY on container_state (not base_labels): it's the
        // single metric the ContainerStopped alert selects on, so other families
        // keep their cardinality. Lets alerting exclude one-shots (restart:no) by
        // label instead of a hand-curated name blacklist.
        state_labels.push(label("restart_policy", &c.restart_policy));
        state_metrics.push(gauge_metric(&state_labels, state_value));

        // Health — one series per container, status label = current state, value 1.
        // c.health is a plain String (normalize_health already collapses unknown → "none").
        let mut health_labels = base_labels.clone();
        health_labels.push(label("status", &c.health));
        health_metrics.push(gauge_metric(&health_labels, 1.0));

        // Lifecycle — gauges
        start_time_metrics.push(gauge_metric(&base_labels, c.started_at));
        last_seen_metrics.push(gauge_metric(&base_labels, c.last_seen));
    }

    families.push(counter_family(
        "container_cpu_usage_seconds_total",
        "Cumulative CPU usage in seconds",
        cpu_metrics,
    ));

    families.push(gauge_family(
        "container_memory_usage_bytes",
        "Current memory usage in bytes (includes cache)",
        mem_usage_metrics,
    ));
    families.push(gauge_family(
        "container_memory_working_set_bytes",
        "Current memory working set in bytes (usage minus cache)",
        mem_working_set_metrics,
    ));
    families.push(gauge_family(
        "container_memory_cache",
        "Memory cache in bytes (inactive_file on cgroup v2, cache on cgroup v1)",
        mem_cache_metrics,
    ));
    families.push(gauge_family(
        "container_memory_limit_bytes",
        "Memory limit in bytes",
        mem_limit_metrics,
    ));

    families.push(counter_family(
        "container_network_receive_bytes_total",
        "Cumulative network bytes received",
        net_rx_metrics,
    ));
    families.push(counter_family(
        "container_network_transmit_bytes_total",
        "Cumulative network bytes transmitted",
        net_tx_metrics,
    ));

    if !blkio_metrics.is_empty() {
        families.push(counter_family(
            "container_blkio_device_usage_total",
            "Cumulative block I/O usage in bytes",
            blkio_metrics,
        ));
    }

    families.push(gauge_family(
        "container_state",
        "Container state (1 = running, 0 = other)",
        state_metrics,
    ));
    families.push(gauge_family(
        "container_health_status",
        "Container health status as reported by Docker (1 for current state)",
        health_metrics,
    ));
    families.push(gauge_family(
        "container_start_time_seconds",
        "Container creation time as Unix timestamp",
        start_time_metrics,
    ));
    families.push(gauge_family(
        "container_last_seen",
        "Last time the container was seen as Unix timestamp",
        last_seen_metrics,
    ));

    families
}

fn base_labels(c: &ContainerMetrics) -> Vec<LabelPair> {
    vec![
        label("id", &c.id),
        label("image", &c.image),
        label("name", &c.name),
    ]
}

fn label(name: &str, value: &str) -> LabelPair {
    let mut lp = LabelPair::default();
    lp.set_name(name.to_owned());
    lp.set_value(value.to_owned());
    lp
}

fn gauge_metric(labels: &[LabelPair], value: f64) -> Metric {
    let mut g = Gauge::default();
    g.set_value(value);

    let mut m = Metric::default();
    m.set_label(labels.to_vec());
    m.set_gauge(g);
    m
}

fn counter_metric(labels: &[LabelPair], value: f64) -> Metric {
    let mut c = Counter::default();
    c.set_value(value);

    let mut m = Metric::default();
    m.set_label(labels.to_vec());
    m.set_counter(c);
    m
}

fn gauge_family(name: &str, help: &str, metrics: Vec<Metric>) -> MetricFamily {
    let mut f = MetricFamily::default();
    f.set_name(name.to_owned());
    f.set_help(help.to_owned());
    f.set_field_type(MetricType::GAUGE);
    f.set_metric(metrics);
    f
}

fn counter_family(name: &str, help: &str, metrics: Vec<Metric>) -> MetricFamily {
    let mut f = MetricFamily::default();
    f.set_name(name.to_owned());
    f.set_help(help.to_owned());
    f.set_field_type(MetricType::COUNTER);
    f.set_metric(metrics);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_container() -> ContainerMetrics {
        ContainerMetrics {
            name: "my-app".into(),
            id: "abc123def456".into(),
            image: "myimage:latest".into(),
            state: "running".into(),
            health: "healthy".into(),
            restart_policy: "always".into(),
            cpu_usage_seconds: 42.5,
            memory_usage_bytes: 104_857_600.0,
            memory_working_set_bytes: 83_886_080.0,
            memory_cache_bytes: 20_971_520.0,
            memory_limit_bytes: 536_870_912.0,
            network: vec![crate::collector::NetworkMetrics {
                interface: "eth0".into(),
                rx_bytes: 1_000_000.0,
                tx_bytes: 500_000.0,
                rx_packets: 1000.0,
                tx_packets: 500.0,
                rx_errors: 0.0,
                tx_errors: 0.0,
                rx_dropped: 0.0,
                tx_dropped: 0.0,
            }],
            block_io_read_bytes: 1_048_576.0,
            block_io_write_bytes: 524_288.0,
            started_at: 1712400000.0,
            last_seen: 1712403600.0,
        }
    }

    #[test]
    fn encode_produces_valid_prometheus_text() {
        let result = ScrapeResult {
            containers: vec![sample_container()],
            scrape_duration_seconds: 0.05,
            docker_up: true,
            inspect_failures_total: 0,
        };

        let output = encode(&result);

        // Counters
        assert!(
            output.contains("# TYPE container_cpu_usage_seconds_total counter"),
            "missing cpu counter type"
        );
        assert!(
            output.contains("container_cpu_usage_seconds_total{"),
            "missing cpu metric"
        );
        assert!(
            output.contains("# TYPE container_network_receive_bytes_total counter"),
            "missing network counter type"
        );
        assert!(
            output.contains("# TYPE container_blkio_device_usage_total counter"),
            "missing blkio counter type"
        );

        // Gauges
        assert!(
            output.contains("# TYPE container_memory_working_set_bytes gauge"),
            "missing memory gauge type"
        );
        assert!(
            output.contains("# TYPE container_state gauge"),
            "missing state gauge type"
        );
        assert!(
            output.contains("# TYPE container_start_time_seconds gauge"),
            "missing start_time gauge type"
        );
        assert!(
            output.contains("# TYPE container_last_seen gauge"),
            "missing last_seen gauge type"
        );

        // Labels
        assert!(output.contains(r#"name="my-app""#), "missing name label");
        assert!(output.contains(r#"id="abc123def456""#), "missing id label");
        assert!(
            output.contains(r#"image="myimage:latest""#),
            "missing image label"
        );
        assert!(
            output.contains(r#"interface="eth0""#),
            "missing interface label"
        );
        assert!(output.contains(r#"state="running""#), "missing state label");

        // Health gauge — one series per container, status label = current state, value 1
        assert!(
            output.contains("# TYPE container_health_status gauge"),
            "missing health gauge type"
        );
        assert!(
            output.contains(r#"container_health_status{"#)
                && output.contains(r#"status="healthy""#),
            "missing health series with status label"
        );
        // Inspect-failure meta counter (always emitted, like docker_exporter_up)
        assert!(
            output.contains("# TYPE docker_exporter_inspect_failures_total counter"),
            "missing inspect-failures counter type"
        );
        assert!(
            output.contains("docker_exporter_inspect_failures_total 0"),
            "missing inspect-failures value"
        );

        // Exporter meta
        assert!(
            output.contains("docker_exporter_up 1"),
            "missing exporter up"
        );
        assert!(
            output.contains("docker_exporter_scrape_duration_seconds"),
            "missing scrape duration"
        );
    }

    #[test]
    fn encode_docker_down() {
        let result = ScrapeResult {
            containers: vec![],
            scrape_duration_seconds: 0.001,
            docker_up: false,
            inspect_failures_total: 0,
        };

        let output = encode(&result);

        assert!(output.contains("docker_exporter_up 0"), "should show down");
        assert!(
            !output.contains("container_cpu_usage_seconds_total"),
            "should have no container metrics"
        );
    }

    #[test]
    fn blkio_skipped_when_zero() {
        let mut container = sample_container();
        container.block_io_read_bytes = 0.0;
        container.block_io_write_bytes = 0.0;

        let result = ScrapeResult {
            containers: vec![container],
            scrape_duration_seconds: 0.01,
            docker_up: true,
            inspect_failures_total: 0,
        };

        let output = encode(&result);
        assert!(
            !output.contains("container_blkio_device_usage_total"),
            "blkio should be omitted when zero"
        );
    }

    #[test]
    fn all_metric_families_present() {
        let result = ScrapeResult {
            containers: vec![sample_container()],
            scrape_duration_seconds: 0.01,
            docker_up: true,
            inspect_failures_total: 0,
        };

        let output = encode(&result);

        let expected = [
            "docker_exporter_up",
            "docker_exporter_scrape_duration_seconds",
            "container_cpu_usage_seconds_total",
            "container_memory_usage_bytes",
            "container_memory_working_set_bytes",
            "container_memory_cache",
            "container_memory_limit_bytes",
            "container_network_receive_bytes_total",
            "container_network_transmit_bytes_total",
            "container_blkio_device_usage_total",
            "container_state",
            "container_start_time_seconds",
            "container_last_seen",
            "container_health_status",
            "docker_exporter_inspect_failures_total",
        ];

        for name in expected {
            assert!(output.contains(name), "metric {name} not found in output");
        }
    }
}
