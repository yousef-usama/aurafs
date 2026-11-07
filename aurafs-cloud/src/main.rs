mod api;
mod consumer;
mod enrichment;

use api::create_health_router;
use consumer::SqsEventConsumer;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured JSON logging (Standard Kubernetes observability)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,aurafs_cloud=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    tracing::info!("Initializing AuraFS Cloud Enrichment Service");

    // 2. Read 12-Factor Configuration from environment
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "eu-central-1".to_string());
    let queue_url = std::env::var("SQS_QUEUE_URL").unwrap_or_else(|_| {
        "https://sqs.eu-central-1.amazonaws.com/339712755065/aurafs-events-dev".to_string()
    });
    let table_name =
        std::env::var("DYNAMODB_TABLE").unwrap_or_else(|_| "aurafs-metadata-dev".to_string());
    let http_port: u16 = std::env::var("HTTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    tracing::info!(
        region = %region,
        queue_url = %queue_url,
        table_name = %table_name,
        http_port = http_port,
        "Loaded runtime cloud configuration"
    );

    // 3. Initialize AWS SDK Clients
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&aws_config);
    let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
    let dynamo_client = aws_sdk_dynamodb::Client::new(&aws_config);

    // 4. Spawn SQS Event Consumer in background task
    let consumer = Arc::new(SqsEventConsumer::new(
        sqs_client,
        s3_client,
        dynamo_client,
        queue_url,
        table_name,
    ));

    let consumer_handle = consumer.clone();
    tokio::spawn(async move {
        consumer_handle.run_loop().await;
    });

    // 5. Start Kubernetes Health Probe HTTP Server
    let app = create_health_router();
    let bind_addr = format!("0.0.0.0:{}", http_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!(address = %bind_addr, "Kubernetes Health & Readiness API listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("AuraFS Cloud Service shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for SIGINT/SIGTERM");
    tracing::info!("Termination signal received; shutting down Kubernetes worker");
}
