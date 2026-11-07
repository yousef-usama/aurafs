variable "enable_irsa" {
  description = "Flag to enable creation of the EKS Pod IRSA IAM role and policy"
  type        = bool
  default     = true
}

variable "github_repository" {
  description = "GitHub repository formatted as owner/repo (e.g., yousef-usama/aurafs)"
  type        = string
}

variable "github_branch" {
  description = "Git branch authorized for deployment role assumption"
  type        = string
  default     = "main"
}

variable "s3_bucket_arn" {
  description = "ARN of the S3 blob bucket"
  type        = string
}

variable "dynamodb_table_arn" {
  description = "ARN of the DynamoDB metadata table"
  type        = string
}

variable "sqs_queue_arn" {
  description = "ARN of the primary SQS queue"
  type        = string
}

variable "ecr_repository_arn" {
  description = "ARN of the ECR repository"
  type        = string
}

variable "eks_oidc_provider_arn" {
  description = "ARN of the EKS OIDC identity provider for IRSA"
  type        = string
  default     = ""
}

variable "eks_oidc_issuer_url" {
  description = "Issuer URL of the EKS OIDC identity provider without https://"
  type        = string
  default     = ""
}

variable "k8s_namespace" {
  description = "Kubernetes namespace for the cloud service"
  type        = string
  default     = "aurafs"
}

variable "k8s_service_account" {
  description = "Kubernetes service account name for the cloud service"
  type        = string
  default     = "aurafs-cloud-sa"
}
