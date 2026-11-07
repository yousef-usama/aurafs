use std::time::Instant;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use crate::cache::CacheManager;

#[derive(Clone)]
pub struct AppState {
    pub cache: CacheManager,
    pub start_time: Instant,
    pub watch_dir: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    uptime_seconds: u64,
    watch_directory: String,
}

#[derive(Deserialize)]
pub struct TagPayload {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct TagSearchQuery {
    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/files", get(list_files_handler))
        .route("/files/{*path}", get(get_file_handler))
        .route("/files/{*path}/tags", post(set_tag_handler))
        .route("/tags/search", get(search_tags_handler))
        .route("/queue", get(get_queue_handler))
        .route("/device", get(get_device_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy",
        service: "aurafs-daemon",
        uptime_seconds: state.start_time.elapsed().as_secs(),
        watch_directory: state.watch_dir,
    })
}

async fn list_files_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.cache.list_all_files() {
        Ok(files) => (StatusCode::OK, Json(ApiResponse { success: true, data: Some(files), error: None })),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}

async fn get_file_handler(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let clean_path = path.trim_start_matches('/');
    match state.cache.get_file_metadata(clean_path) {
        Ok(Some(meta)) => (StatusCode::OK, Json(ApiResponse { success: true, data: Some(meta), error: None })),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse { success: false, data: None, error: Some("File not tracked".to_string()) }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}

async fn set_tag_handler(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    Json(payload): Json<TagPayload>,
) -> impl IntoResponse {
    let clean_path = path.trim_start_matches('/');
    match state.cache.set_file_tag(clean_path, payload.key, payload.value).await {
        Ok(()) => (
            StatusCode::OK,
            Json(ApiResponse::<String> { success: true, data: Some("Tag recorded".to_string()), error: None }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<String> { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}

async fn search_tags_handler(
    State(state): State<AppState>,
    Query(query): Query<TagSearchQuery>,
) -> impl IntoResponse {
    match state.cache.find_files_by_tag(&query.key, &query.value) {
        Ok(matched) => (StatusCode::OK, Json(ApiResponse { success: true, data: Some(matched), error: None })),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}

async fn get_queue_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.cache.get_pending_sync_operations() {
        Ok(ops) => (StatusCode::OK, Json(ApiResponse { success: true, data: Some(ops), error: None })),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}

async fn get_device_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.cache.get_device_state() {
        Ok(Some(info)) => (StatusCode::OK, Json(ApiResponse { success: true, data: Some(info), error: None })),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse { success: false, data: None, error: Some("Device uninitialized".to_string()) }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}
