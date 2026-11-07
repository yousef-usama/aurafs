variable "bucket_name" {
  description = "Unique name for the S3 blob storage bucket"
  type        = string
}

variable "sqs_queue_arn" {
  description = "ARN of the SQS queue that receives object creation events"
  type        = string
}

variable "environment" {
  description = "Environment name (e.g. dev, prod)"
  type        = string
}
