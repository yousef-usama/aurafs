output "queue_arn" {
  description = "ARN of the primary SQS queue"
  value       = aws_sqs_queue.main.arn
}

output "queue_url" {
  description = "URL of the primary SQS queue"
  value       = aws_sqs_queue.main.id
}

output "dlq_arn" {
  description = "ARN of the dead-letter SQS queue"
  value       = aws_sqs_queue.dlq.arn
}
