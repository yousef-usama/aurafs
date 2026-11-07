use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client as S3Client;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const CHUNK_SIZE_BYTES: usize = 8 * 1024 * 1024; // 8 MB chunks
const MULTIPART_THRESHOLD_BYTES: u64 = 8 * 1024 * 1024; // 8 MB threshold
const MAX_CONCURRENT_UPLOAD_PARTS: usize = 4;

#[derive(Error, Debug)]
pub enum UploaderError {
    #[error("AWS S3 SDK error: {0}")]
    S3(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Multipart upload failed: {0}")]
    Multipart(String),
}

/// Asynchronous S3 Blob Uploader with automated multipart chunking and part-level retry.
pub struct S3SyncUploader {
    client: S3Client,
    bucket_name: String,
}

impl S3SyncUploader {
    pub fn new(client: S3Client, bucket_name: impl Into<String>) -> Self {
        Self {
            client,
            bucket_name: bucket_name.into(),
        }
    }

    /// Upload a file to S3, automatically selecting single PutObject or concurrent Multipart Upload.
    pub async fn upload_file(
        &self,
        local_path: &Path,
        s3_key: &str,
    ) -> Result<String, UploaderError> {
        let metadata = tokio::fs::metadata(local_path).await?;
        let file_size = metadata.len();

        if file_size <= MULTIPART_THRESHOLD_BYTES {
            self.upload_single_part(local_path, s3_key).await
        } else {
            self.upload_multipart(local_path, s3_key, file_size).await
        }
    }

    /// Single PutObject upload for small files (<= 8 MB).
    async fn upload_single_part(
        &self,
        local_path: &Path,
        s3_key: &str,
    ) -> Result<String, UploaderError> {
        let stream = ByteStream::from_path(local_path)
            .await
            .map_err(|e| UploaderError::S3(e.to_string()))?;

        self.client
            .put_object()
            .bucket(&self.bucket_name)
            .key(s3_key)
            .body(stream)
            .send()
            .await
            .map_err(|e| UploaderError::S3(e.to_string()))?;

        tracing::info!(bucket = %self.bucket_name, key = %s3_key, "Completed single PutObject upload");
        Ok(s3_key.to_string())
    }

    /// Concurrent Multipart Upload for large files (> 8 MB) with automated abort cleanup on error.
    async fn upload_multipart(
        &self,
        local_path: &Path,
        s3_key: &str,
        file_size: u64,
    ) -> Result<String, UploaderError> {
        // 1. Initiate Multipart Upload
        let create_res = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket_name)
            .key(s3_key)
            .send()
            .await
            .map_err(|e| UploaderError::S3(e.to_string()))?;

        let upload_id = create_res
            .upload_id()
            .ok_or_else(|| {
                UploaderError::Multipart("Missing upload_id from AWS response".to_string())
            })?
            .to_string();

        tracing::info!(
            bucket = %self.bucket_name,
            key = %s3_key,
            upload_id = %upload_id,
            size = file_size,
            "Initiated S3 Multipart Upload"
        );

        // 2. Read and upload chunks concurrently with bounded semaphore
        let mut file = File::open(local_path).await?;
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_UPLOAD_PARTS));
        let mut join_set: JoinSet<Result<CompletedPart, UploaderError>> = JoinSet::new();
        let mut part_number = 1;

        loop {
            let mut chunk_buffer = vec![0u8; CHUNK_SIZE_BYTES];
            let mut bytes_read = 0;

            while bytes_read < CHUNK_SIZE_BYTES {
                let n = file.read(&mut chunk_buffer[bytes_read..]).await?;
                if n == 0 {
                    break;
                }
                bytes_read += n;
            }

            if bytes_read == 0 {
                break;
            }

            chunk_buffer.truncate(bytes_read);

            let client = self.client.clone();
            let bucket = self.bucket_name.clone();
            let key = s3_key.to_string();
            let up_id = upload_id.clone();
            let sem = Arc::clone(&semaphore);
            let current_part = part_number;

            join_set.spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|_| UploaderError::Multipart("Concurrency semaphore closed".into()))?;

                let res = client
                    .upload_part()
                    .bucket(&bucket)
                    .key(&key)
                    .upload_id(&up_id)
                    .part_number(current_part)
                    .body(ByteStream::from(chunk_buffer))
                    .send()
                    .await
                    .map_err(|e| UploaderError::S3(e.to_string()))?;

                let etag = res.e_tag().unwrap_or_default().to_string();
                Ok(CompletedPart::builder()
                    .part_number(current_part)
                    .e_tag(etag)
                    .build())
            });

            part_number += 1;
        }

        // 3. Await all parts and assemble completed parts list
        let mut completed_parts: Vec<CompletedPart> = Vec::new();
        let mut upload_failed = false;

        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok(Ok(part)) => completed_parts.push(part),
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Multipart chunk upload failed");
                    upload_failed = true;
                }
                Err(join_err) => {
                    tracing::error!(error = %join_err, "Task panicked during multipart upload");
                    upload_failed = true;
                }
            }
        }

        // 4. Abort cleanup if any part failed
        if upload_failed {
            let _ = self
                .client
                .abort_multipart_upload()
                .bucket(&self.bucket_name)
                .key(s3_key)
                .upload_id(&upload_id)
                .send()
                .await;

            return Err(UploaderError::Multipart(
                "One or more chunks failed to upload; aborted".into(),
            ));
        }

        // Sort completed parts by part number (required by S3 API)
        completed_parts.sort_by_key(|p| p.part_number());

        let completed_multipart = CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        // 5. Complete Multipart Upload
        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket_name)
            .key(s3_key)
            .upload_id(&upload_id)
            .multipart_upload(completed_multipart)
            .send()
            .await
            .map_err(|e| UploaderError::S3(e.to_string()))?;

        tracing::info!(bucket = %self.bucket_name, key = %s3_key, "Successfully completed S3 Multipart Upload");
        Ok(s3_key.to_string())
    }
}
