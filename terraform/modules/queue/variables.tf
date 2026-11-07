variable "queue_name" {
  description = "Name for the main SQS event processing queue"
  type        = string
}

variable "s3_bucket_arn" {
  description = "ARN of the S3 bucket permitted to push event notifications"
  type        = string
}
