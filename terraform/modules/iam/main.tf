data "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"
}

# ---------------------------------------------------------------------------------------------------------------------
# 1. GitHub Actions CI/CD Deployment IAM Role (OIDC Authenticated)
# ---------------------------------------------------------------------------------------------------------------------
resource "aws_iam_role" "github_actions" {
  name        = "aurafs-github-actions-deploy"
  description = "IAM Role assumed by GitHub Actions workflows via OIDC token"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Federated = data.aws_iam_openid_connect_provider.github.arn
        }
        Action = "sts:AssumeRoleWithWebIdentity"
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" = "repo:${var.github_repository}:ref:refs/heads/${var.github_branch}"
          }
        }
      }
    ]
  })
}

resource "aws_iam_policy" "github_actions_deploy" {
  name        = "aurafs-github-actions-deploy-policy"
  description = "Permissions required for GitHub Actions to deploy infrastructure, ECR images, and workloads"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "ECRTokenAuth"
        Effect = "Allow"
        Action = [
          "ecr:GetAuthorizationToken"
        ]
        Resource = "*"
      },
      {
        Sid    = "ECRImagePush"
        Effect = "Allow"
        Action = [
          "ecr:BatchCheckLayerAvailability",
          "ecr:GetDownloadUrlForLayer",
          "ecr:BatchGetImage",
          "ecr:PutImage",
          "ecr:InitiateLayerUpload",
          "ecr:UploadLayerPart",
          "ecr:CompleteLayerUpload"
        ]
        Resource = var.ecr_repository_arn
      },
      {
        Sid    = "EKSClusterDescribe"
        Effect = "Allow"
        Action = [
          "eks:DescribeCluster",
          "eks:ListClusters"
        ]
        Resource = "*"
      },
      {
        Sid    = "InfrastructureManagement"
        Effect = "Allow"
        Action = [
          "s3:*",
          "dynamodb:*",
          "sqs:*",
          "cloudfront:*"
        ]
        Resource = [
          var.s3_bucket_arn,
          "${var.s3_bucket_arn}/*",
          var.dynamodb_table_arn,
          "${var.dynamodb_table_arn}/*",
          var.sqs_queue_arn
        ]
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "github_actions_attach" {
  role       = aws_iam_role.github_actions.name
  policy_arn = aws_iam_policy.github_actions_deploy.arn
}

# ---------------------------------------------------------------------------------------------------------------------
# 2. Kubernetes Pod Service Account IAM Role (IRSA - Least Privilege)
# ---------------------------------------------------------------------------------------------------------------------
resource "aws_iam_role" "cloud_pod_irsa" {
  count       = var.eks_oidc_provider_arn != "" ? 1 : 0
  name        = "aurafs-cloud-pod-irsa"
  description = "IAM Role assumed by aurafs-cloud Kubernetes pods via IAM Roles for Service Accounts (IRSA)"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Federated = var.eks_oidc_provider_arn
        }
        Action = "sts:AssumeRoleWithWebIdentity"
        Condition = {
          StringEquals = {
            "${var.eks_oidc_issuer_url}:sub" = "system:serviceaccount:${var.k8s_namespace}:${var.k8s_service_account}"
            "${var.eks_oidc_issuer_url}:aud" = "sts.amazonaws.com"
          }
        }
      }
    ]
  })
}

resource "aws_iam_policy" "cloud_pod_policy" {
  count       = var.eks_oidc_provider_arn != "" ? 1 : 0
  name        = "aurafs-cloud-pod-policy"
  description = "Runtime permissions for aurafs-cloud enrichment service"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "S3BlobReadWrite"
        Effect = "Allow"
        Action = [
          "s3:GetObject",
          "s3:PutObject"
        ]
        Resource = "${var.s3_bucket_arn}/*"
      },
      {
        Sid    = "SQSConsumeMessages"
        Effect = "Allow"
        Action = [
          "sqs:ReceiveMessage",
          "sqs:DeleteMessage",
          "sqs:GetQueueAttributes",
          "sqs:GetQueueUrl"
        ]
        Resource = var.sqs_queue_arn
      },
      {
        Sid    = "DynamoDBReadWriteMetadata"
        Effect = "Allow"
        Action = [
          "dynamodb:PutItem",
          "dynamodb:UpdateItem",
          "dynamodb:GetItem",
          "dynamodb:Query",
          "dynamodb:BatchWriteItem"
        ]
        Resource = [
          var.dynamodb_table_arn,
          "${var.dynamodb_table_arn}/index/*"
        ]
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "cloud_pod_attach" {
  count      = var.eks_oidc_provider_arn != "" ? 1 : 0
  role       = aws_iam_role.cloud_pod_irsa[0].name
  policy_arn = aws_iam_policy.cloud_pod_policy[0].arn
}
