output "github_actions_role_arn" {
  description = "ARN of the IAM role for GitHub Actions OIDC authentication"
  value       = aws_iam_role.github_actions.arn
}

output "cloud_pod_irsa_role_arn" {
  description = "ARN of the IAM role for EKS pods via IRSA"
  value       = try(aws_iam_role.cloud_pod_irsa[0].arn, "")
}
