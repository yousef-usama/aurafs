use crate::enrichment::traits::{ContentAnalysisResult, ContentAnalyzer};
use async_trait::async_trait;

/// Fast, zero-allocation MIME type analyzer based on file magic numbers.
pub struct MimeDetector;

#[async_trait]
impl ContentAnalyzer for MimeDetector {
    fn name(&self) -> &'static str {
        "MimeDetector"
    }

    async fn analyze(&self, data: &[u8], filename: &str) -> anyhow::Result<ContentAnalysisResult> {
        let detected_mime = if let Some(kind) = infer::get(data) {
            Some(kind.mime_type().to_string())
        } else if filename.ends_with(".md") || filename.ends_with(".markdown") {
            Some("text/markdown".to_string())
        } else if filename.ends_with(".json") {
            Some("application/json".to_string())
        } else if filename.ends_with(".txt") {
            Some("text/plain".to_string())
        } else {
            Some("application/octet-stream".to_string())
        };

        Ok(ContentAnalysisResult {
            mime_type: detected_mime,
            verified_hash: None,
            attributes: std::collections::HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mime_detection() {
        let detector = MimeDetector;

        // Markdown file
        let md_res = detector
            .analyze(b"# AuraFS Cloud", "architecture.md")
            .await
            .unwrap();
        assert_eq!(md_res.mime_type.as_deref(), Some("text/markdown"));

        // JSON file
        let json_res = detector
            .analyze(b"{\"key\": \"val\"}", "data.json")
            .await
            .unwrap();
        assert_eq!(json_res.mime_type.as_deref(), Some("application/json"));
    }
}
