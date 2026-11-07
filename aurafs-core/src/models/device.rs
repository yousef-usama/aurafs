use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Information identifying this local device instance in the AuraFS network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique identifier for this device (UUIDv4).
    pub device_id: String,
    /// Human-readable device label (e.g., hostname).
    pub device_name: String,
    /// When this device was first initialized.
    pub registered_at: DateTime<Utc>,
    /// Last successful push or pull synchronization timestamp.
    pub last_sync_at: Option<DateTime<Utc>>,
}

impl DeviceInfo {
    pub fn new(device_id: impl Into<String>, device_name: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            device_name: device_name.into(),
            registered_at: Utc::now(),
            last_sync_at: None,
        }
    }
}
