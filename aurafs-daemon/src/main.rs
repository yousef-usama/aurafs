mod api;
mod cache;
mod config;
mod sync;
mod watcher;

use api::{create_router, AppState};
use aurafs_core::models::{DeviceInfo, FileMetadata, SyncOperation, SyncStatus};
use cache::CacheManager;
use chrono::Utc;
use config::DaemonConfig;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use sync::{DynamoDbMetadataWriter, S3SyncUploader, SyncEngine};
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use watcher::{DebouncedFileEvent, FileWatcherService};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,aurafs_daemon=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 2. Parse CLI arguments or load configuration defaults
    let args: Vec<String> = std::env::args().collect();
    let mut config = DaemonConfig::default();

    for i in 1..args.len() {
        if args[i] == "--watch-dir" && i + 1 < args.len() {
            config.watch_directory = PathBuf::from(&args[i + 1]);
        } else if args[i] == "--api-port" && i + 1 < args.len() {
            if let Ok(port) = args[i + 1].parse::<u16>() {
                config.api_port = port;
            }
        }
    }

    tracing::info!(
        watch_directory = %config.watch_directory.display(),
        database_path = %config.database_path.display(),
        api_port = config.api_port,
        "Starting AuraFS Local Synchronization Daemon"
    );

    // 3. Initialize embedded L1 cache (redb)
    let cache = CacheManager::new(&config.database_path)?;

    // 4. Ensure local device identity is registered
    let device_id = match cache.get_device_state()? {
        Some(info) => info.device_id,
        None => {
            let hostname =
                std::env::var("HOSTNAME").unwrap_or_else(|_| "local-machine".to_string());
            let id = uuid::Uuid::new_v4().to_string();
            let device_info = DeviceInfo::new(id.clone(), hostname);
            cache.save_device_state(device_info).await?;
            tracing::info!(device_id = %id, "Registered new local device identity in L1 cache");
            id
        }
    };

    // 5. Initialize AWS SDK clients (S3 & DynamoDB)
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&aws_config);
    let dynamo_client = aws_sdk_dynamodb::Client::new(&aws_config);

    let uploader = Arc::new(S3SyncUploader::new(s3_client, &config.s3_bucket));
    let metadata_writer = Arc::new(DynamoDbMetadataWriter::new(
        dynamo_client,
        &config.dynamodb_table,
        device_id,
    ));

    // 6. Spawn background AWS synchronization engine
    let sync_engine = Arc::new(SyncEngine::new(
        cache.clone(),
        uploader,
        metadata_writer,
        config.watch_directory.clone(),
    ));
    sync_engine.start_worker();

    // 7. Setup file system watcher and debouncer pipeline
    let (event_tx, mut event_rx) = mpsc::channel::<DebouncedFileEvent>(1024);
    let watcher_service = FileWatcherService::new(
        config.watch_directory.clone(),
        config.debounce_ms,
        config.max_hashing_concurrency,
        event_tx,
    );
    watcher_service.start().await?;

    // 8. Spawn event processor task (watcher events -> L1 cache & write-ahead sync queue)
    let processor_cache = cache.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                DebouncedFileEvent::CreateOrModify {
                    relative_path,
                    absolute_path: _,
                    blake3_hash,
                    size_bytes,
                } => {
                    let existing = processor_cache.get_file_metadata(&relative_path);
                    let should_update = match existing {
                        Ok(Some(meta)) => meta.blake3_hash != blake3_hash,
                        _ => true,
                    };

                    if should_update {
                        let meta = FileMetadata::new(
                            &relative_path,
                            size_bytes,
                            &blake3_hash,
                            Utc::now(),
                            Utc::now(),
                        );

                        // Persist to L1 cache
                        if let Err(e) = processor_cache.upsert_file_metadata(meta).await {
                            tracing::error!(error = %e, path = %relative_path, "Failed to cache file metadata");
                            continue;
                        }

                        // Commit operation to write-ahead sync queue
                        let op =
                            SyncOperation::new_put(0, &relative_path, &blake3_hash, size_bytes);
                        match processor_cache.enqueue_sync_operation(op).await {
                            Ok(seq_id) => {
                                tracing::info!(
                                    seq_id = seq_id,
                                    path = %relative_path,
                                    hash = %blake3_hash,
                                    size = size_bytes,
                                    "Committed file event to sync queue"
                                );
                            }
                            Err(e) => {
                                tracing::error!(error = %e, path = %relative_path, "Failed to enqueue sync operation");
                            }
                        }
                    }
                }
                DebouncedFileEvent::Delete { relative_path } => {
                    tracing::info!(path = %relative_path, "Detected file deletion; updating L1 cache");
                    let _ = processor_cache
                        .update_sync_status(&relative_path, SyncStatus::Deleted)
                        .await;
                    let op = SyncOperation::new_delete(0, &relative_path);
                    let _ = processor_cache.enqueue_sync_operation(op).await;
                }
            }
        }
    });

    // 9. Initialize and bind Axum HTTP administration server
    let app_state = AppState {
        cache: cache.clone(),
        start_time: Instant::now(),
        watch_dir: config.watch_directory.display().to_string(),
    };

    let router = create_router(app_state);
    let bind_addr = format!("127.0.0.1:{}", config.api_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!(address = %bind_addr, "AuraFS Local Administration HTTP API listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("AuraFS Daemon gracefully shut down");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for SIGINT/Ctrl+C");
    tracing::info!("Shutdown signal received; flushing pending operations");
}
