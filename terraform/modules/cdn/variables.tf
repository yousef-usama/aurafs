variable "s3_bucket_id" {
  description = "The ID of the S3 origin bucket"
  type        = string
}

variable "s3_bucket_arn" {
  description = "The ARN of the S3 origin bucket"
  type        = string
}

variable "s3_bucket_regional_domain_name" {
  description = "Regional domain name of the S3 origin bucket"
  type        = string
}

variable "environment" {
  description = "Environment name (dev, prod)"
  type        = string
}
