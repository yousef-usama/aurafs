use crate::enrichment::{ContentAnalyzer, ExifExtractor, HashVerifier, MimeDetector};
use aurafs_core::dynamo_schema::{file_pk, FILE_META_SK, PK_ATTR, SK_ATTR};
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_sqs::Client as SqsClient;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub struct SqsEventConsumer {
    sqs_client: SqsClient,
    s3_client: S3Client,
    dynamo_client: DynamoClient,
    queue_url: String,
    table_name: String,
    analyzers: Vec<Box<dyn ContentAnalyzer>>,
}

impl SqsEventConsumer {
    pub fn new(
        sqs_client: SqsClient,
        s3_client: S3Client,
        dynamo_client: DynamoClient,
        queue_url: impl Into<String>,
        table_name: impl Into<String>,
    ) -> Self {
        let analyzers: Vec<Box<dyn ContentAnalyzer>> = vec![
            Box::new(MimeDetector),
            Box::new(HashVerifier),
            Box::new(ExifExtractor),
        ];

        Self {
            sqs_client,
            s3_client,
            dynamo_client,
            queue_url: queue_url.into(),
            table_name: table_name.into(),
            analyzers,
        }
    }

    /// Continuous long-polling consumer worker loop.
    pub async fn run_loop(self: Arc<Self>) {
        tracing::info!(queue_url = %self.queue_url, "Starting SQS Event Consumer long-polling loop");

        loop {
            let receive_res = self
                .sqs_client
                .receive_message()
                .queue_url(&self.queue_url)
                .max_number_of_messages(5)
                .wait_time_seconds(20) // Long polling reduces empty-receive AWS billing costs
                .send()
                .await;

            match receive_res {
                Ok(resp) => {
                    let messages = resp.messages();
                    for msg in messages {
                        if let Some(body) = msg.body() {
                            if let Err(e) = self.process_event_payload(body).await {
                                tracing::error!(error = %e, "Failed to process S3 event from message body");
                                continue;
                            }
                        }

                        // Acknowledge and delete processed message from SQS
                        if let Some(receipt_handle) = msg.receipt_handle() {
                            let _ = self
                                .sqs_client
                                .delete_message()
                                .queue_url(&self.queue_url)
                                .receipt_handle(receipt_handle)
                                .send()
                                .await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "SQS receive error; backing off 5s");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Parse S3 Event Notification JSON, download blob, run analyzers, and persist enriched metadata.
    async fn process_event_payload(&self, payload: &str) -> anyhow::Result<()> {
        let v: Value = serde_json::from_str(payload)?;

        let records = match v.get("Records").and_then(|r| r.as_array()) {
            Some(recs) => recs,
            None => return Ok(()),
        };

        for record in records {
            let s3_info = match record.get("s3") {
                Some(info) => info,
                None => continue,
            };

            let bucket_name = match s3_info
                .get("bucket")
                .and_then(|b| b.get("name"))
                .and_then(|n| n.as_str())
            {
                Some(b) => b,
                None => continue,
            };

            let s3_key = match s3_info
                .get("object")
                .and_then(|o| o.get("key"))
                .and_then(|k| k.as_str())
            {
                Some(k) => k,
                None => continue,
            };

            // URL-decode S3 object key
            let decoded_key = urlencoding::decode(s3_key)
                .unwrap_or(std::borrow::Cow::Borrowed(s3_key))
                .to_string();

            // Extract relative filename: format is files/<hash>/<filename>
            let filename = decoded_key.split('/').next_back().unwrap_or("file");

            tracing::info!(bucket = bucket_name, key = %decoded_key, "Processing newly uploaded blob for enrichment");

            // 1. Download file bytes from S3
            let get_obj_res = self
                .s3_client
                .get_object()
                .bucket(bucket_name)
                .key(&decoded_key)
                .send()
                .await?;

            let body_bytes = get_obj_res.body.collect().await?.to_vec();

            // 2. Execute content analyzer pipeline
            let mut detected_mime = None;
            let mut verified_hash = None;
            let mut extracted_attributes = std::collections::HashMap::new();

            for analyzer in &self.analyzers {
                tracing::debug!(analyzer = analyzer.name(), "Executing content analyzer");
                if let Ok(res) = analyzer.analyze(&body_bytes, filename).await {
                    if let Some(m) = res.mime_type {
                        detected_mime = Some(m);
                    }
                    if let Some(h) = res.verified_hash {
                        verified_hash = Some(h);
                    }
                    for (k, v) in res.attributes {
                        extracted_attributes.insert(k, v);
                    }
                }
            }

            // 3. Update DynamoDB with enriched metadata
            let pk = file_pk(filename);
            let mut update_expression = "SET enriched_at = :now".to_string();
            let mut expr_vals = std::collections::HashMap::new();
            expr_vals.insert(
                ":now".to_string(),
                AttributeValue::S(chrono::Utc::now().to_rfc3339()),
            );

            if let Some(ref mime) = detected_mime {
                update_expression.push_str(", mime_type = :mime");
                expr_vals.insert(":mime".to_string(), AttributeValue::S(mime.clone()));
            }

            if let Some(ref hash) = verified_hash {
                update_expression.push_str(", verified_hash = :hash");
                expr_vals.insert(":hash".to_string(), AttributeValue::S(hash.clone()));
            }

            let _ = self
                .dynamo_client
                .update_item()
                .table_name(&self.table_name)
                .key(PK_ATTR, AttributeValue::S(pk))
                .key(SK_ATTR, AttributeValue::S(FILE_META_SK.to_string()))
                .update_expression(update_expression)
                .set_expression_attribute_values(Some(expr_vals))
                .send()
                .await;

            tracing::info!(
                key = %decoded_key,
                mime = ?detected_mime,
                hash = ?verified_hash,
                attributes = extracted_attributes.len(),
                "Persisted enriched metadata to DynamoDB"
            );
        }

        Ok(())
    }
}
