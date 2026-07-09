//! Internal library backing the `docker-exporter` binary.
//!
//! The collector pulls per-container stats from the Docker daemon via
//! `bollard`, computes cgroup-v2-correct working-set memory, and emits
//! cAdvisor-compatible Prometheus text. The axum router exposes
//! `/metrics`, `/health`, and `/ready` and is the only public surface.
//!
//! This crate is **not a stable public library**: items below are kept
//! `pub` so the binary entry point and integration tests in this file
//! can reach them, and `#[doc(hidden)]` so they are not advertised to
//! external dependents. Treat them as private — semver does not apply.

#[doc(hidden)]
pub mod collector;
#[doc(hidden)]
pub mod config;
#[doc(hidden)]
pub mod health;
#[doc(hidden)]
pub mod metrics;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use bollard::Docker;
use tower_http::timeout::TimeoutLayer;

#[doc(hidden)]
pub struct AppState {
    pub docker: Docker,
    pub exclude: config::ExcludeMatcher,
    pub inspect_failures: std::sync::atomic::AtomicU64,
    pub stats_failures: std::sync::atomic::AtomicU64,
}

/// Per-request timeout for the HTTP server.
///
/// Defense-in-depth against slow clients holding TCP connections open
/// indefinitely (slowloris). Comfortably above the 5 s per-container stats
/// timeout multiplied by the worst-case scrape duration we've measured
/// (~3 s on 30+ containers), so legitimate scrapes never trip it. The
/// Docker `HEALTHCHECK` (native `--health` TCP probe, 2 s budget) doesn't
/// touch this HTTP path at all, so it never interacts with this timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[doc(hidden)]
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .with_state(state)
}

async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let result = collector::collect(
        &state.docker,
        &state.exclude,
        &state.inspect_failures,
        &state.stats_failures,
    )
    .await;
    let body = metrics::encode(&result);

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;

    fn test_state() -> Option<Arc<AppState>> {
        let docker = Docker::connect_with_socket_defaults().ok()?;
        Some(Arc::new(AppState {
            docker,
            exclude: config::ExcludeMatcher::parse("").expect("empty exclude is always valid"),
            inspect_failures: std::sync::atomic::AtomicU64::new(0),
            stats_failures: std::sync::atomic::AtomicU64::new(0),
        }))
    }

    #[tokio::test]
    async fn health_returns_200_when_docker_available() {
        let Some(state) = test_state() else {
            eprintln!("skipping: Docker socket not available");
            return;
        };

        if state.docker.ping().await.is_err() {
            eprintln!("skipping: Docker daemon not reachable");
            return;
        }

        let server = TestServer::new(build_router(state)).unwrap();
        let response = server.get("/health").await;
        response.assert_status_ok();
        response.assert_text("ok");
    }

    #[tokio::test]
    async fn ready_returns_200_when_docker_available() {
        let Some(state) = test_state() else {
            eprintln!("skipping: Docker socket not available");
            return;
        };

        if state.docker.ping().await.is_err() {
            eprintln!("skipping: Docker daemon not reachable");
            return;
        }

        let server = TestServer::new(build_router(state)).unwrap();
        let response = server.get("/ready").await;
        response.assert_status_ok();
        response.assert_text("ready");
    }

    #[tokio::test]
    async fn metrics_returns_prometheus_text_when_docker_available() {
        let Some(state) = test_state() else {
            eprintln!("skipping: Docker socket not available");
            return;
        };

        if state.docker.ping().await.is_err() {
            eprintln!("skipping: Docker daemon not reachable");
            return;
        }

        let server = TestServer::new(build_router(state)).unwrap();
        let response = server.get("/metrics").await;
        response.assert_status_ok();

        let body = response.text();
        // Only assert metrics that are emitted regardless of the live container set —
        // CI's Docker daemon has zero containers, so per-container families
        // (container_health_status, container_cpu_*, …) are legitimately absent here.
        // container_health_status is covered by the metrics.rs encode test (synthetic
        // container). docker_exporter_inspect_failures_total is a meta counter emitted
        // before the empty-container early-return, so it's always present.
        assert!(body.contains("docker_exporter_up 1"));
        assert!(body.contains("docker_exporter_scrape_duration_seconds"));
        assert!(
            body.contains("docker_exporter_inspect_failures_total"),
            "inspect-failures counter missing"
        );
        assert!(
            body.contains("docker_exporter_stats_failures_total"),
            "stats-failures counter missing"
        );
    }
}
