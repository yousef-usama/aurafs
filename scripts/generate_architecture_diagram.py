#!/usr/bin/env python3
"""
AuraFS Architecture Diagram Generator
Generates presentation-ready cloud architecture diagrams using the Python 'diagrams' package.
"""

import sys
try:
    from diagrams import Diagram, Cluster, Edge
    from diagrams.aws.storage import S3
    from diagrams.aws.database import Dynamodb
    from diagrams.aws.network import CloudFront
    from diagrams.aws.integration import SQS
    from diagrams.aws.compute import EKS
    from diagrams.aws.security import IAMRole
    from diagrams.onprem.client import Client
except ImportError:
    print("Error: 'diagrams' package is not installed. Run: pip3 install diagrams")
    sys.exit(1)

def generate_diagram():
    graph_attr = {
        "fontsize": "16",
        "bgcolor": "transparent",
        "pad": "0.5"
    }

    with Diagram("AuraFS System Architecture", show=False, filename="docs/architecture-overview", outformat="png", graph_attr=graph_attr):
        with Cluster("Local Client Machine"):
            local_fs = Client("Watched Directory\n~/aurafs/")
            daemon = Client("AuraFS Daemon\n(Rust, tokio, redb)")
            local_fs >> Edge(label="notify events") >> daemon

        with Cluster("AWS Cloud (eu-central-1)"):
            with Cluster("Edge & CDN"):
                cdn = CloudFront("CloudFront\n(Edge Cache / Downloads)")

            with Cluster("Storage & Messaging"):
                s3_bucket = S3("S3 Bucket\n(Raw File Blobs)")
                sqs_events = SQS("SQS Queue + DLQ\n(Object Created)")
                s3_bucket >> Edge(label="event notification") >> sqs_events

            with Cluster("State & Metadata"):
                dynamo = Dynamodb("DynamoDB Table\n(Single-Table Design)")

            with Cluster("Kubernetes Compute (EKS)"):
                eks_cluster = EKS("EKS Cluster\naurafs-cloud pods")
                irsa_role = IAMRole("IAM Role (IRSA)\nLeast Privilege")
                irsa_role - Edge(style="dotted") - eks_cluster

        # Data flows
        daemon >> Edge(label="multipart upload") >> s3_bucket
        daemon >> Edge(label="push/pull metadata") >> dynamo
        daemon << Edge(label="cached pull") << cdn
        cdn >> Edge(label="origin fetch") >> s3_bucket
        sqs_events >> Edge(label="consume") >> eks_cluster
        eks_cluster >> Edge(label="enrich metadata") >> dynamo

if __name__ == "__main__":
    generate_diagram()
    print("Generated docs/architecture-overview.png successfully.")
