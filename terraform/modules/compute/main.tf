# ---------------------------------------------------------------------------------------------------------------------
# 1. ECR Repository for aurafs-cloud Container Images
# ---------------------------------------------------------------------------------------------------------------------
#checkov:skip=CKV_AWS_136: AWS-managed AES256 encryption used in place of customer-managed KMS key
#checkov:skip=CKV_AWS_51: Mutable tags enabled for rapid portfolio developer iteration
resource "aws_ecr_repository" "cloud_service" {
  name                 = "aurafs-cloud"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  encryption_configuration {
    encryption_type = "AES256"
  }
}

resource "aws_ecr_lifecycle_policy" "cloud_service" {
  repository = aws_ecr_repository.cloud_service.name

  policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Keep only last 10 container images to optimize storage costs"
        selection = {
          tagStatus   = "any"
          countType   = "imageCountMoreThan"
          countNumber = 10
        }
        action = {
          type = "expire"
        }
      }
    ]
  })
}

# ---------------------------------------------------------------------------------------------------------------------
# 2. EKS Cluster Control Plane IAM Role
# ---------------------------------------------------------------------------------------------------------------------
resource "aws_iam_role" "cluster_role" {
  name = "aurafs-eks-cluster-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "eks.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "cluster_policy" {
  policy_arn = "arn:aws:iam::aws/policy/AmazonEKSClusterPolicy"
  role       = aws_iam_role.cluster_role.name
}

resource "aws_iam_role_policy_attachment" "vpc_resource_controller" {
  policy_arn = "arn:aws:iam::aws/policy/AmazonEKSVPCResourceController"
  role       = aws_iam_role.cluster_role.name
}

# ---------------------------------------------------------------------------------------------------------------------
# 3. EKS Cluster Provisioning
# ---------------------------------------------------------------------------------------------------------------------
#checkov:skip=CKV_AWS_39: Public endpoint enabled to allow kubectl management without dedicated bastion/VPN
#checkov:skip=CKV_AWS_58: Secrets encryption with KMS CMK omitted to avoid monthly KMS fees
resource "aws_eks_cluster" "main" {
  name                      = var.cluster_name
  role_arn                  = aws_iam_role.cluster_role.arn
  version                   = "1.31"
  enabled_cluster_log_types = ["api", "audit", "authenticator", "controllerManager", "scheduler"]

  vpc_config {
    subnet_ids              = var.subnet_ids
    endpoint_private_access = true
    endpoint_public_access  = true
  }

  depends_on = [
    aws_iam_role_policy_attachment.cluster_policy,
    aws_iam_role_policy_attachment.vpc_resource_controller,
  ]
}

# ---------------------------------------------------------------------------------------------------------------------
# 4. EKS OIDC Identity Provider for IAM Roles for Service Accounts (IRSA)
# ---------------------------------------------------------------------------------------------------------------------
data "tls_certificate" "eks" {
  url = aws_eks_cluster.main.identity[0].oidc[0].issuer
}

resource "aws_iam_openid_connect_provider" "eks" {
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = [data.tls_certificate.eks.certificates[0].sha1_fingerprint]
  url             = aws_eks_cluster.main.identity[0].oidc[0].issuer
}

# ---------------------------------------------------------------------------------------------------------------------
# 5. EKS Managed Node Group IAM Role
# ---------------------------------------------------------------------------------------------------------------------
resource "aws_iam_role" "node_role" {
  name = "aurafs-eks-node-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "node_worker" {
  policy_arn = "arn:aws:iam::aws/policy/AmazonEKSWorkerNodePolicy"
  role       = aws_iam_role.node_role.name
}

resource "aws_iam_role_policy_attachment" "node_cni" {
  policy_arn = "arn:aws:iam::aws/policy/AmazonEKS_CNI_Policy"
  role       = aws_iam_role.node_role.name
}

resource "aws_iam_role_policy_attachment" "node_ecr" {
  policy_arn = "arn:aws:iam::aws/policy/AmazonEC2ContainerRegistryReadOnly"
  role       = aws_iam_role.node_role.name
}

# ---------------------------------------------------------------------------------------------------------------------
# 6. EKS Managed Node Group
# ---------------------------------------------------------------------------------------------------------------------
#checkov:skip=CKV_AWS_226: IMDSv2 hop limit check skipped for managed node group defaults
resource "aws_eks_node_group" "workers" {
  cluster_name    = aws_eks_cluster.main.name
  node_group_name = "aurafs-workers-${var.environment}"
  node_role_arn   = aws_iam_role.node_role.arn
  subnet_ids      = var.private_subnet_ids

  scaling_config {
    desired_size = 1
    max_size     = 3
    min_size     = 1
  }

  instance_types = ["t3.medium"]
  capacity_type  = "ON_DEMAND"

  labels = {
    role = "worker"
    env  = var.environment
  }

  depends_on = [
    aws_iam_role_policy_attachment.node_worker,
    aws_iam_role_policy_attachment.node_cni,
    aws_iam_role_policy_attachment.node_ecr,
  ]
}

# ---------------------------------------------------------------------------------------------------------------------
# 7. EKS Core Addons
# ---------------------------------------------------------------------------------------------------------------------
resource "aws_eks_addon" "vpc_cni" {
  cluster_name = aws_eks_cluster.main.name
  addon_name   = "vpc-cni"
}

resource "aws_eks_addon" "coredns" {
  cluster_name = aws_eks_cluster.main.name
  addon_name   = "coredns"
  depends_on   = [aws_eks_node_group.workers]
}

resource "aws_eks_addon" "kube_proxy" {
  cluster_name = aws_eks_cluster.main.name
  addon_name   = "kube-proxy"
}
