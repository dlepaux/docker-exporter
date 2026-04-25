use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::AppState;

pub async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.docker.ping().await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "docker unreachable"),
    }
}

pub async fn ready_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.docker.ping().await {
        Ok(_) => (StatusCode::OK, "ready"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "not ready"),
    }
}
