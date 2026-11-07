output "distribution_id" {
  description = "The identifier of the CloudFront distribution"
  value       = aws_cloudfront_distribution.cdn.id
}

output "distribution_arn" {
  description = "The ARN of the CloudFront distribution"
  value       = aws_cloudfront_distribution.cdn.arn
}

output "domain_name" {
  description = "Domain name of CloudFront distribution used for edge-accelerated file downloads"
  value       = aws_cloudfront_distribution.cdn.domain_name
}
