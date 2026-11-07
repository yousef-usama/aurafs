variable "aws_region" {
  description = "Target AWS region for infrastructure deployment"
  type        = string
  default     = "eu-central-1"
}

variable "environment" {
  description = "Deployment environment name (e.g., dev, prod)"
  type        = string
  default     = "dev"
}

variable "project_name" {
  description = "Base project name used in resource naming and tags"
  type        = string
  default     = "aurafs"
}

variable "github_repository" {
  description = "GitHub repository owner and name formatted as <owner>/<repo>"
  type        = string
  default     = "yousef-usama/aurafs"
}

variable "github_branch" {
  description = "Target GitHub branch allowed to assume the deployment OIDC IAM role"
  type        = string
  default     = "main"
}
