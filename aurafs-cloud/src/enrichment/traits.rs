use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The outcome of running a content analyzer over a file payload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentAnalysisResult {
    /// Detected MIME type (e.g. `image/jpeg`, `application/pdf`).
    pub mime_type: Option<String>,
    /// Verified BLAKE3 checksum.
    pub verified_hash: Option<String>,
    /// Extensible metadata tags (e.g. EXIF camera details, dimensions, AI classifications).
    pub attributes: HashMap<String, String>,
}

/// Extensible trait for file content inspectors.
///
/// Enables seamless addition of AI inference models, optical character recognition (OCR),
/// and multimodal embedding generators without altering core queue ingestion mechanics.
#[async_trait]
pub trait ContentAnalyzer: Send + Sync {
    /// Identifier name of the analyzer.
    fn name(&self) -> &'static str;

    /// Inspect raw file bytes and extract structured metadata attributes.
    async fn analyze(&self, data: &[u8], filename: &str) -> anyhow::Result<ContentAnalysisResult>;
}
