use crate::models::sync::SyncStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata describing a file tracked by AuraFS.
///
/// This struct is serialized with `bincode` for the local L1 cache (`redb`)
/// and serialized with `serde_json` for API responses and DynamoDB attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Relative POSIX path from the root watched directory (e.g., `docs/readme.md`).
    pub relative_path: String,
    /// Size of the file in bytes.
    pub size_bytes: u64,
    /// Cryptographic BLAKE3 hash represented as a 64-character lowercase hexadecimal string.
    pub blake3_hash: String,
    /// Detected MIME type (e.g., `text/markdown`, `image/png`), if known.
    pub mime_type: Option<String>,
    /// UTC timestamp of file creation.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp of last local file modification.
    pub modified_at: DateTime<Utc>,
    /// Amazon S3 object key (e.g., `files/<hash>/<basename>`).
    pub s3_key: String,
    /// Current local-to-cloud synchronization status.
    pub sync_status: SyncStatus,
    /// Extensible user and AI-generated key-value tags.
    pub tags: HashMap<String, String>,
}

impl FileMetadata {
    /// Construct a new [`FileMetadata`] instance with pending sync status.
    pub fn new(
        relative_path: impl Into<String>,
        size_bytes: u64,
        blake3_hash: impl Into<String>,
        created_at: DateTime<Utc>,
        modified_at: DateTime<Utc>,
    ) -> Self {
        let relative_path = relative_path.into();
        let blake3_hash = blake3_hash.into();
        let basename = std::path::Path::new(&relative_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        let s3_key = format!("files/{}/{}", blake3_hash, basename);

        Self {
            relative_path,
            size_bytes,
            blake3_hash,
            mime_type: None,
            created_at,
            modified_at,
            s3_key,
            sync_status: SyncStatus::Pending,
            tags: HashMap::new(),
        }
    }
}
