variable "cluster_name" {
  description = "Name of the EKS cluster"
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC"
  type        = string
}

variable "subnet_ids" {
  description = "Subnet IDs for the EKS control plane (public and private subnets)"
  type        = list(string)
}

variable "private_subnet_ids" {
  description = "Private subnet IDs for the EKS worker nodes"
  type        = list(string)
}

variable "environment" {
  description = "Environment name (dev, prod)"
  type        = string
}
