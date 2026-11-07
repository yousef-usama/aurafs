use aurafs_core::dynamo_schema::{
    file_pk, gsi1_device_pk, FILE_META_SK, GSI1_PK_ATTR, GSI1_SK_ATTR, PK_ATTR, SK_ATTR,
};
use aurafs_core::models::FileMetadata;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetadataWriterError {
    #[error("AWS DynamoDB SDK error: {0}")]
    DynamoDb(String),
    #[error("Conditional check failed (concurrent write or stale update rejected)")]
    ConditionalCheckFailed,
}

/// Asynchronous DynamoDB Metadata Sync Writer implementing Single-Table Design.
pub struct DynamoDbMetadataWriter {
    client: DynamoClient,
    table_name: String,
    device_id: String,
}

impl DynamoDbMetadataWriter {
    pub fn new(
        client: DynamoClient,
        table_name: impl Into<String>,
        device_id: impl Into<String>,
    ) -> Self {
        Self {
            client,
            table_name: table_name.into(),
            device_id: device_id.into(),
        }
    }

    /// Put file metadata item into DynamoDB with conditional idempotency protection.
    pub async fn put_file_metadata(&self, meta: &FileMetadata) -> Result<(), MetadataWriterError> {
        let pk = file_pk(&meta.relative_path);
        let modified_at_iso = meta.modified_at.to_rfc3339();
        let gsi1_pk = gsi1_device_pk(&self.device_id);

        let mut item = HashMap::new();
        item.insert(PK_ATTR.to_string(), AttributeValue::S(pk));
        item.insert(
            SK_ATTR.to_string(),
            AttributeValue::S(FILE_META_SK.to_string()),
        );
        item.insert(
            "relative_path".to_string(),
            AttributeValue::S(meta.relative_path.clone()),
        );
        item.insert(
            "blake3_hash".to_string(),
            AttributeValue::S(meta.blake3_hash.clone()),
        );
        item.insert(
            "size_bytes".to_string(),
            AttributeValue::N(meta.size_bytes.to_string()),
        );
        item.insert("s3_key".to_string(), AttributeValue::S(meta.s3_key.clone()));
        item.insert(
            "sync_status".to_string(),
            AttributeValue::S(
                serde_json::to_string(&meta.sync_status)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
            ),
        );
        item.insert(
            "modified_at".to_string(),
            AttributeValue::S(modified_at_iso.clone()),
        );
        item.insert(GSI1_PK_ATTR.to_string(), AttributeValue::S(gsi1_pk));
        item.insert(
            GSI1_SK_ATTR.to_string(),
            AttributeValue::S(modified_at_iso.clone()),
        );

        if let Some(ref mime) = meta.mime_type {
            item.insert("mime_type".to_string(), AttributeValue::S(mime.clone()));
        }

        // Conditional check: Ensure we only write if the file record does not exist OR the incoming modification is strictly newer
        let condition_expression =
            "attribute_not_exists(pk) OR modified_at <= :incoming_modified_at";

        let mut expr_vals = HashMap::new();
        expr_vals.insert(
            ":incoming_modified_at".to_string(),
            AttributeValue::S(modified_at_iso),
        );

        let res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .condition_expression(condition_expression)
            .set_expression_attribute_values(Some(expr_vals))
            .send()
            .await;

        match res {
            Ok(_) => {
                tracing::info!(table = %self.table_name, path = %meta.relative_path, "Recorded file metadata in DynamoDB");
                Ok(())
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("ConditionalCheckFailedException") {
                    tracing::warn!(path = %meta.relative_path, "Rejected stale metadata write via conditional check");
                    Err(MetadataWriterError::ConditionalCheckFailed)
                } else {
                    Err(MetadataWriterError::DynamoDb(err_str))
                }
            }
        }
    }

    /// Mark a file as deleted in DynamoDB via tombstone record.
    pub async fn mark_file_deleted(&self, relative_path: &str) -> Result<(), MetadataWriterError> {
        let pk = file_pk(relative_path);

        let res = self
            .client
            .update_item()
            .table_name(&self.table_name)
            .key(PK_ATTR, AttributeValue::S(pk))
            .key(SK_ATTR, AttributeValue::S(FILE_META_SK.to_string()))
            .update_expression("SET sync_status = :status, modified_at = :now")
            .expression_attribute_values(":status", AttributeValue::S("DELETED".to_string()))
            .expression_attribute_values(":now", AttributeValue::S(chrono::Utc::now().to_rfc3339()))
            .send()
            .await;

        match res {
            Ok(_) => {
                tracing::info!(table = %self.table_name, path = %relative_path, "Marked file as DELETED in DynamoDB");
                Ok(())
            }
            Err(e) => Err(MetadataWriterError::DynamoDb(e.to_string())),
        }
    }
}
