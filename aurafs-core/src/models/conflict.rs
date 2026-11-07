use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Record documenting a concurrent edit or resurrection conflict between devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRecord {
    /// Unique conflict identifier (UUIDv4).
    pub conflict_id: String,
    /// Canonical relative path where the conflict arose.
    pub relative_path: String,
    /// BLAKE3 hash of the local version.
    pub local_hash: String,
    /// BLAKE3 hash of the remote version.
    pub remote_hash: String,
    /// Modification timestamp of local copy.
    pub local_modified_at: DateTime<Utc>,
    /// Modification timestamp of remote copy.
    pub remote_modified_at: DateTime<Utc>,
    /// Disk path where the conflicted version was preserved (e.g. `notes.conflict.device123.txt`).
    pub conflict_file_path: String,
    /// True if the user or automated policy has marked this conflict resolved.
    pub resolved: bool,
}
