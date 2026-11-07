use crate::enrichment::traits::{ContentAnalysisResult, ContentAnalyzer};
use async_trait::async_trait;
use aurafs_core::hashing::hash_bytes;

/// Cryptographic content integrity verifier.
pub struct HashVerifier;

#[async_trait]
impl ContentAnalyzer for HashVerifier {
    fn name(&self) -> &'static str {
        "HashVerifier"
    }

    async fn analyze(&self, data: &[u8], _filename: &str) -> anyhow::Result<ContentAnalysisResult> {
        let computed_hash = hash_bytes(data);

        Ok(ContentAnalysisResult {
            mime_type: None,
            verified_hash: Some(computed_hash),
            attributes: std::collections::HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurafs_core::hashing::hash_bytes;

    #[tokio::test]
    async fn test_hash_verifier() {
        let verifier = HashVerifier;
        let data = b"AuraFS Cloud Content Integrity Test";
        let res = verifier.analyze(data, "sample.bin").await.unwrap();
        assert_eq!(
            res.verified_hash.as_deref(),
            Some(hash_bytes(data).as_str())
        );
    }
}
