//! Binary entry point for `docker-exporter`.
//!
//! Loads configuration from the environment, connects to the Docker socket,
//! verifies reachability with a startup `ping`, builds the axum router from
//! `docker_exporter::build_router`, and serves until SIGINT/SIGTERM. All
//! observability flows through `tracing`; log filtering is taken from
//! `RUST_LOG` if set, otherwise derived from the configured `LOG_LEVEL`.
//!
//! Invoked with `--health`, the binary instead performs a one-shot liveness
//! probe (TCP-connect to its own configured port on loopback) and exits 0 if
//! the server is up, 1 otherwise. This is the container `HEALTHCHECK` path —
//! it replaces the previous `wget` dependency so the runtime image can be a
//! static musl binary on distroless with no shell or extra tooling.

use std::net::SocketAddr;
use std::process;
use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use tracing_subscriber::EnvFilter;

use docker_exporter::config::Config;
use docker_exporter::{AppState, build_router};

/// Timeout for the `--health` TCP probe. Comfortably under the Docker
/// `HEALTHCHECK --timeout` so the probe always returns its own verdict
/// rather than being killed mid-connect.
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() {
    // Health-probe mode: a one-shot TCP connect to our own port, then exit.
    // Handled before any server setup so it stays a cheap, dependency-free
    // path. A bad/missing config here means we cannot know the port, so we
    // report unhealthy (exit 1) rather than panic.
    if std::env::args().skip(1).any(|arg| arg == "--health") {
        let addr = match Config::from_env() {
            Ok(config) => loopback_addr(config.listen_addr),
            Err(_) => process::exit(1),
        };
        process::exit(i32::from(!check_health(addr, HEALTH_PROBE_TIMEOUT).await));
    }

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("configuration error: {err}");
            process::exit(1);
        }
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("docker_exporter={}", config.log_level)));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!(
        listen_addr = %config.listen_addr,
        "starting docker-exporter"
    );

    let docker = match Docker::connect_with_socket_defaults() {
        Ok(docker) => docker,
        Err(err) => {
            tracing::error!(%err, "failed to connect to Docker socket");
            process::exit(1);
        }
    };

    // Verify Docker is reachable at startup
    if let Err(err) = docker.ping().await {
        tracing::error!(%err, "Docker daemon is not reachable");
        process::exit(1);
    }

    let version = docker.version().await.ok();
    if let Some(ref v) = version {
        tracing::info!(
            docker_version = v.version.as_deref().unwrap_or("unknown"),
            api_version = v.api_version.as_deref().unwrap_or("unknown"),
            "connected to Docker daemon"
        );
    }

    if !config.exclude.is_empty() {
        tracing::info!(
            exclude = ?config.exclude.patterns(),
            "container exclusion filter active"
        );
    }

    let state = Arc::new(AppState {
        docker,
        exclude: config.exclude,
        inspect_failures: std::sync::atomic::AtomicU64::new(0),
    });
    let app = build_router(state);

    let listener = match tokio::net::TcpListener::bind(config.listen_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(%err, addr = %config.listen_addr, "failed to bind");
            process::exit(1);
        }
    };

    tracing::info!(addr = %config.listen_addr, "HTTP server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|err| {
            tracing::error!(%err, "server error");
            process::exit(1);
        });

    tracing::info!("shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }

    tracing::info!("shutdown signal received");
}

/// Rewrite a bind address to its loopback equivalent on the same port.
///
/// The server binds `0.0.0.0` (or `::`) but the health probe must dial a
/// concrete reachable address from inside the container — `127.0.0.1` /
/// `::1`, never the wildcard.
fn loopback_addr(addr: SocketAddr) -> SocketAddr {
    let ip = match addr {
        SocketAddr::V4(_) => std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        SocketAddr::V6(_) => std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
    };
    SocketAddr::new(ip, addr.port())
}

/// One-shot liveness probe: can we open a TCP connection to `addr` within
/// `timeout`? Returns `true` only on a successful connect. A connect error
/// or a timeout both mean unhealthy (`false`) — the caller maps that to a
/// non-zero exit code.
async fn check_health(addr: SocketAddr, timeout: Duration) -> bool {
    matches!(
        tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::net::TcpListener;

    #[test]
    fn loopback_addr_rewrites_wildcard_v4_keeping_port() {
        let bound: SocketAddr = "0.0.0.0:9713".parse().unwrap();
        let probe = loopback_addr(bound);
        assert_eq!(probe.ip(), std::net::Ipv4Addr::LOCALHOST);
        assert_eq!(probe.port(), 9713);
    }

    #[test]
    fn loopback_addr_rewrites_wildcard_v6_keeping_port() {
        let bound: SocketAddr = "[::]:9713".parse().unwrap();
        let probe = loopback_addr(bound);
        assert_eq!(probe.ip(), std::net::Ipv6Addr::LOCALHOST);
        assert_eq!(probe.port(), 9713);
    }

    #[tokio::test]
    async fn check_health_is_true_when_listener_is_bound() {
        // Bind an ephemeral loopback port and leave the listener alive: the
        // probe must connect successfully.
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        assert!(check_health(addr, Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn check_health_is_false_when_port_is_closed() {
        // Bind to grab a free port, then drop the listener so the port is
        // closed: the probe must fail (connection refused).
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        assert!(!check_health(addr, Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn check_health_times_out_without_hanging() {
        // A non-routable TEST-NET-1 address (RFC 5737) never completes the
        // connect, so the probe must rely on its own timeout. We assert both
        // the verdict (unhealthy) and that it returned well within a small
        // multiple of the budget — proving the timeout bounds the wait
        // instead of hanging on the OS connect default (~tens of seconds).
        let unreachable: SocketAddr = "192.0.2.1:9713".parse().unwrap();
        let budget = Duration::from_millis(200);

        let start = std::time::Instant::now();
        let healthy = check_health(unreachable, budget).await;
        let elapsed = start.elapsed();

        assert!(!healthy);
        assert!(
            elapsed < budget * 5,
            "probe took {elapsed:?}, expected to be bounded by the {budget:?} timeout"
        );
    }
}
