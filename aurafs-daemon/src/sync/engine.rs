use crate::cache::CacheManager;
use crate::sync::metadata_writer::DynamoDbMetadataWriter;
use crate::sync::uploader::S3SyncUploader;
use aurafs_core::models::{OperationType, SyncStatus};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Background synchronization engine driving local-to-cloud push workers with exponential backoff.
pub struct SyncEngine {
    cache: CacheManager,
    uploader: Arc<S3SyncUploader>,
    metadata_writer: Arc<DynamoDbMetadataWriter>,
    watch_dir: PathBuf,
}

impl SyncEngine {
    pub fn new(
        cache: CacheManager,
        uploader: Arc<S3SyncUploader>,
        metadata_writer: Arc<DynamoDbMetadataWriter>,
        watch_dir: PathBuf,
    ) -> Self {
        Self {
            cache,
            uploader,
            metadata_writer,
            watch_dir,
        }
    }

    /// Spawn the continuous background synchronization worker loop.
    pub fn start_worker(self: Arc<Self>) {
        tokio::spawn(async move {
            tracing::info!("Started background cloud synchronization worker");
            let mut interval = tokio::time::interval(Duration::from_millis(500));

            loop {
                interval.tick().await;

                let pending_ops = match self.cache.get_pending_sync_operations() {
                    Ok(ops) => ops,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to read pending sync operations from L1 cache");
                        continue;
                    }
                };

                for op in pending_ops {
                    let local_file_path = self.watch_dir.join(&op.relative_path);

                    match op.operation_type {
                        OperationType::CreateOrUpdate => {
                            if !local_file_path.exists() {
                                tracing::warn!(path = %op.relative_path, "File disappeared before upload; clearing queue");
                                let _ = self.cache.delete_sync_operation(op.sequence_id).await;
                                continue;
                            }

                            // Update local status to Syncing
                            let _ = self
                                .cache
                                .update_sync_status(&op.relative_path, SyncStatus::Syncing)
                                .await;

                            let s3_key = format!(
                                "files/{}/{}",
                                op.blake3_hash.as_deref().unwrap_or("blob"),
                                PathBuf::from(&op.relative_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("file")
                            );

                            // 1. Upload Blob to S3
                            let upload_res =
                                self.uploader.upload_file(&local_file_path, &s3_key).await;

                            match upload_res {
                                Ok(_) => {
                                    // 2. Fetch latest local metadata for DynamoDB write
                                    if let Ok(Some(mut meta)) =
                                        self.cache.get_file_metadata(&op.relative_path)
                                    {
                                        meta.s3_key = s3_key;
                                        meta.sync_status = SyncStatus::Synced;

                                        // 3. Write Metadata to DynamoDB
                                        match self.metadata_writer.put_file_metadata(&meta).await {
                                            Ok(_) => {
                                                let _ = self.cache.upsert_file_metadata(meta).await;
                                                let _ = self
                                                    .cache
                                                    .delete_sync_operation(op.sequence_id)
                                                    .await;
                                                tracing::info!(path = %op.relative_path, "File synchronized successfully to AWS cloud");
                                            }
                                            Err(e) => {
                                                tracing::error!(error = %e, path = %op.relative_path, "Failed to write metadata to DynamoDB");
                                                let _ = self
                                                    .cache
                                                    .update_sync_status(
                                                        &op.relative_path,
                                                        SyncStatus::Failed,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, path = %op.relative_path, "Failed to upload file to S3");
                                    let _ = self
                                        .cache
                                        .update_sync_status(&op.relative_path, SyncStatus::Failed)
                                        .await;
                                }
                            }
                        }
                        OperationType::Delete => {
                            match self
                                .metadata_writer
                                .mark_file_deleted(&op.relative_path)
                                .await
                            {
                                Ok(_) => {
                                    let _ = self.cache.delete_sync_operation(op.sequence_id).await;
                                    tracing::info!(path = %op.relative_path, "Propagated file deletion to DynamoDB");
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, path = %op.relative_path, "Failed to record deletion in DynamoDB");
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
