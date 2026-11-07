output "bucket_id" {
  description = "The name of the S3 blob storage bucket"
  value       = aws_s3_bucket.blobs.id
}

output "bucket_arn" {
  description = "The ARN of the S3 blob storage bucket"
  value       = aws_s3_bucket.blobs.arn
}

output "bucket_regional_domain_name" {
  description = "Regional domain name of S3 bucket for CloudFront origin configuration"
  value       = aws_s3_bucket.blobs.bucket_regional_domain_name
}
