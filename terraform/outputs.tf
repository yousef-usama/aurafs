output "s3_bucket_name" {
  description = "Name of the S3 blob storage bucket"
  value       = module.storage.bucket_id
}

output "dynamodb_table_name" {
  description = "Name of the DynamoDB metadata single-table"
  value       = module.metadata.table_name
}

output "sqs_queue_url" {
  description = "URL of the primary SQS queue"
  value       = module.queue.queue_url
}

output "cloudfront_domain_name" {
  description = "Domain name of the CloudFront CDN distribution"
  value       = module.cdn.domain_name
}

output "eks_cluster_name" {
  description = "Name of the EKS cluster"
  value       = module.compute.cluster_name
}

output "eks_cluster_endpoint" {
  description = "API server endpoint of the EKS cluster"
  value       = module.compute.cluster_endpoint
}

output "ecr_repository_url" {
  description = "URL of the ECR container repository"
  value       = module.compute.ecr_repository_url
}

output "github_actions_role_arn" {
  description = "ARN of the IAM role assumed by GitHub Actions via OIDC"
  value       = module.iam.github_actions_role_arn
}

output "cloud_pod_irsa_role_arn" {
  description = "ARN of the IAM role assumed by Kubernetes pods via IRSA"
  value       = module.iam.cloud_pod_irsa_role_arn
}
