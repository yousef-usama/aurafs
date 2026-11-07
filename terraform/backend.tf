terraform {
  backend "s3" {
    bucket         = "aurafs-tf-state-339712755065"
    key            = "dev/terraform.tfstate"
    region         = "eu-central-1"
    dynamodb_table = "aurafs-tf-locks"
    encrypt        = true
  }
}
