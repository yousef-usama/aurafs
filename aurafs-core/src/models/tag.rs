use serde::{Deserialize, Serialize};

/// Key-value tag attached to a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// Tag namespace or key (e.g., `project`, `ai.category`, `camera.model`).
    pub key: String,
    /// Value associated with the key (e.g., `portfolio`, `landscape`, `sony-a7iv`).
    pub value: String,
}

impl Tag {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Query filter for searching files by tag in local cache or DynamoDB GSI2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagQuery {
    pub key: String,
    pub value: Option<String>,
}
