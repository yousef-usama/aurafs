use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthStatus {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

pub fn create_health_router() -> Router {
    Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
}

async fn healthz_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(HealthStatus {
            status: "healthy",
            service: "aurafs-cloud",
            version: env!("CARGO_PKG_VERSION"),
        }),
    )
}

async fn readyz_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(HealthStatus {
            status: "ready",
            service: "aurafs-cloud",
            version: env!("CARGO_PKG_VERSION"),
        }),
    )
}
