//! Binary entry point for `docker-exporter`.
//!
//! Loads configuration from the environment, connects to the Docker socket,
//! verifies reachability with a startup `ping`, builds the axum router from
//! `docker_exporter::build_router`, and serves until SIGINT/SIGTERM. All
//! observability flows through `tracing`; log filtering is taken from
//! `RUST_LOG` if set, otherwise derived from the configured `LOG_LEVEL`.

use std::process;
use std::sync::Arc;

use bollard::Docker;
use tracing_subscriber::EnvFilter;

use docker_exporter::config::Config;
use docker_exporter::{AppState, build_router};

#[tokio::main]
async fn main() {
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

    if !config.exclude_containers.is_empty() {
        tracing::info!(
            exclude = ?config.exclude_containers,
            "container exclusion filter active"
        );
    }

    let state = Arc::new(AppState {
        docker,
        exclude_containers: config.exclude_containers,
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
