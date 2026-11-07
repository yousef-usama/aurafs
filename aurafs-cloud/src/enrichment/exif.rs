use crate::enrichment::traits::{ContentAnalysisResult, ContentAnalyzer};
use async_trait::async_trait;
use exif::{In, Reader, Tag};
use std::collections::HashMap;
use std::io::Cursor;

/// Image EXIF metadata extractor.
pub struct ExifExtractor;

#[async_trait]
impl ContentAnalyzer for ExifExtractor {
    fn name(&self) -> &'static str {
        "ExifExtractor"
    }

    async fn analyze(&self, data: &[u8], _filename: &str) -> anyhow::Result<ContentAnalysisResult> {
        let mut attributes = HashMap::new();

        let mut cursor = Cursor::new(data);
        if let Ok(exif) = Reader::new().read_from_container(&mut cursor) {
            if let Some(field) = exif.get_field(Tag::Make, In::PRIMARY) {
                attributes.insert("exif.make".to_string(), field.display_value().to_string());
            }
            if let Some(field) = exif.get_field(Tag::Model, In::PRIMARY) {
                attributes.insert("exif.model".to_string(), field.display_value().to_string());
            }
            if let Some(field) = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
                attributes.insert(
                    "exif.datetime".to_string(),
                    field.display_value().to_string(),
                );
            }
            if let Some(field) = exif.get_field(Tag::PixelXDimension, In::PRIMARY) {
                attributes.insert("exif.width".to_string(), field.display_value().to_string());
            }
            if let Some(field) = exif.get_field(Tag::PixelYDimension, In::PRIMARY) {
                attributes.insert("exif.height".to_string(), field.display_value().to_string());
            }
        }

        Ok(ContentAnalysisResult {
            mime_type: None,
            verified_hash: None,
            attributes,
        })
    }
}
