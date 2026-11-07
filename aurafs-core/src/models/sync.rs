use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// High-level synchronization state of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SyncStatus {
    /// Queued for synchronization to cloud.
    Pending,
    /// Currently being uploaded or downloaded.
    Syncing,
    /// In sync between local cache and AWS cloud.
    Synced,
    /// Failed with an error; queued for exponential backoff retry.
    Failed,
    /// Marked for deletion / deleted from cloud.
    Deleted,
}

/// The direction of synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    /// Local machine pushing changes to AWS S3 & DynamoDB.
    Push,
    /// Local machine pulling changes down from AWS S3.
    Pull,
}

/// The specific file system operation requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// New file created or existing file modified.
    CreateOrUpdate,
    /// File removed from the watched directory.
    Delete,
}

/// A persistent unit of work in the sync queue table.
///
/// Written to `redb` before attempting network I/O to ensure zero lost updates
/// across unexpected daemon crashes or network outages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncOperation {
    /// Monotonically increasing sequence number serving as primary key in redb.
    pub sequence_id: u64,
    /// Relative POSIX path of the file.
    pub relative_path: String,
    /// File system action to execute.
    pub operation_type: OperationType,
    /// Expected BLAKE3 hash when operation was queued (None for Delete).
    pub blake3_hash: Option<String>,
    /// File size in bytes at time of queuing.
    pub size_bytes: u64,
    /// When the event was committed to the write-ahead sync queue.
    pub queued_at: DateTime<Utc>,
    /// Number of failed upload attempts.
    pub retry_count: u32,
    /// Description of the latest error message, if any.
    pub last_error: Option<String>,
}

impl SyncOperation {
    pub fn new_put(
        sequence_id: u64,
        relative_path: impl Into<String>,
        blake3_hash: impl Into<String>,
        size_bytes: u64,
    ) -> Self {
        Self {
            sequence_id,
            relative_path: relative_path.into(),
            operation_type: OperationType::CreateOrUpdate,
            blake3_hash: Some(blake3_hash.into()),
            size_bytes,
            queued_at: Utc::now(),
            retry_count: 0,
            last_error: None,
        }
    }

    pub fn new_delete(sequence_id: u64, relative_path: impl Into<String>) -> Self {
        Self {
            sequence_id,
            relative_path: relative_path.into(),
            operation_type: OperationType::Delete,
            blake3_hash: None,
            size_bytes: 0,
            queued_at: Utc::now(),
            retry_count: 0,
            last_error: None,
        }
    }
}
