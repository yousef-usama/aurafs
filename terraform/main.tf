data "aws_caller_identity" "current" {}

locals {
  account_id   = data.aws_caller_identity.current.account_id
  cluster_name = "${var.project_name}-eks-${var.environment}"
}

# 1. Networking Infrastructure (VPC, Subnets, NAT, IGW)
module "network" {
  source       = "./modules/network"
  environment  = var.environment
  cluster_name = local.cluster_name
}

# 2. Compute Infrastructure (EKS, Node Group, ECR)
module "compute" {
  source             = "./modules/compute"
  cluster_name       = local.cluster_name
  environment        = var.environment
  vpc_id             = module.network.vpc_id
  subnet_ids         = concat(module.network.public_subnet_ids, module.network.private_subnet_ids)
  private_subnet_ids = module.network.private_subnet_ids
}

# 3. Asynchronous Messaging (SQS Event Queue + DLQ)
module "queue" {
  source        = "./modules/queue"
  queue_name    = "${var.project_name}-events-${var.environment}"
  s3_bucket_arn = module.storage.bucket_arn
}

# 4. Blob Storage (S3 with versioning, encryption, and SQS event triggers)
module "storage" {
  source        = "./modules/storage"
  bucket_name   = "${var.project_name}-blobs-${local.account_id}-${var.environment}"
  sqs_queue_arn = module.queue.queue_arn
  environment   = var.environment
}

# 5. Metadata State (DynamoDB Single-Table Design)
module "metadata" {
  source     = "./modules/metadata"
  table_name = "${var.project_name}-metadata-${var.environment}"
}

# 6. Content Delivery Network (CloudFront Edge Distribution)
module "cdn" {
  source                         = "./modules/cdn"
  s3_bucket_id                   = module.storage.bucket_id
  s3_bucket_arn                  = module.storage.bucket_arn
  s3_bucket_regional_domain_name = module.storage.bucket_regional_domain_name
  environment                    = var.environment
}

# 7. Identity & Access Management (GitHub OIDC & EKS Pod IRSA)
module "iam" {
  source                = "./modules/iam"
  github_repository     = var.github_repository
  github_branch         = var.github_branch
  s3_bucket_arn         = module.storage.bucket_arn
  dynamodb_table_arn    = module.metadata.table_arn
  sqs_queue_arn         = module.queue.queue_arn
  ecr_repository_arn    = module.compute.ecr_repository_arn
  eks_oidc_provider_arn = module.compute.oidc_provider_arn
  eks_oidc_issuer_url   = module.compute.oidc_issuer_url
}
