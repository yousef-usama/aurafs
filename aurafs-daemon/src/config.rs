use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Runtime configuration for the AuraFS local synchronization daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Absolute path to the local directory watched and synchronized by AuraFS.
    pub watch_directory: PathBuf,
    /// Absolute path to the local embedded `redb` database file.
    pub database_path: PathBuf,
    /// Port for the local Axum HTTP administration and inspection API.
    pub api_port: u16,
    /// Event settling quiet period in milliseconds before hashing and queuing.
    pub debounce_ms: u64,
    /// Maximum number of concurrent CPU-bound BLAKE3 hashing tasks.
    pub max_hashing_concurrency: usize,
    /// AWS Region for cloud synchronization.
    pub aws_region: String,
    /// Name of the DynamoDB metadata single-table.
    pub dynamodb_table: String,
    /// Name of the S3 blob storage bucket.
    pub s3_bucket: String,
    /// CloudFront CDN domain name for edge-accelerated file downloads.
    pub cloudfront_domain: String,
    /// Polling interval in seconds for pull synchronization.
    pub pull_interval_seconds: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let watch_directory = PathBuf::from(&home).join("aurafs");
        let database_path = PathBuf::from(&home).join(".aurafs").join("state.redb");

        Self {
            watch_directory,
            database_path,
            api_port: 7700,
            debounce_ms: 500,
            max_hashing_concurrency: num_cpus(),
            aws_region: "eu-central-1".to_string(),
            dynamodb_table: "aurafs-metadata-dev".to_string(),
            s3_bucket: "aurafs-blobs-339712755065-dev".to_string(),
            cloudfront_domain: String::new(),
            pull_interval_seconds: 30,
        }
    }
}

impl DaemonConfig {
    /// Load configuration from a TOML file, falling back to defaults for unspecified keys.
    #[allow(dead_code)]
    pub fn load_or_default(path: Option<&Path>) -> Self {
        if let Some(p) = path {
            if p.exists() {
                if let Ok(content) = std::fs::read_to_string(p) {
                    if let Ok(cfg) = toml::from_str::<DaemonConfig>(&content) {
                        return cfg;
                    }
                }
            }
        }
        Self::default()
    }
}

/// Helper function to detect available logical CPU cores.
pub fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
