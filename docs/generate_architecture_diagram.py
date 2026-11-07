#!/usr/bin/env python3
"""
AuraFS Distributed Cloud Architecture Diagram Generator
Renders a Lucidchart-style architecture diagram utilizing the Python diagrams library,
backed by Graphviz, with official AWS icons and typography.
"""

from diagrams import Diagram, Cluster, Edge
from diagrams.aws.storage import S3
from diagrams.aws.network import CloudFront, VPC, InternetGateway, NATGateway
from diagrams.aws.database import Dynamodb
from diagrams.aws.integration import SQS
from diagrams.aws.compute import EKS, ECR
from diagrams.aws.security import IAMRole, IAM
from diagrams.onprem.client import User
from diagrams.programming.language import Rust
from diagrams.generic.storage import Storage

# Lucidchart style graph and cluster attributes
graph_attr = {
    "fontsize": "28",
    "fontname": "Helvetica-Bold",
    "bgcolor": "#FFFFFF",
    "pad": "0.6",
    "splines": "ortho",
    "nodesep": "0.7",
    "ranksep": "0.9",
}

node_attr = {
    "fontname": "Helvetica",
    "fontsize": "12",
    "fontcolor": "#232F3E",
}

edge_attr = {
    "fontname": "Helvetica",
    "fontsize": "10",
    "fontcolor": "#545B64",
    "color": "#7D8998",
    "penwidth": "1.5",
}

cluster_client_attr = {
    "fontsize": "16",
    "fontname": "Helvetica-Bold",
    "fontcolor": "#16191F",
    "bgcolor": "#F4F6F8",
    "style": "rounded,filled",
    "pencolor": "#D5DBDB",
    "penwidth": "2.0",
}

cluster_aws_attr = {
    "fontsize": "18",
    "fontname": "Helvetica-Bold",
    "fontcolor": "#232F3E",
    "bgcolor": "#FAFAFA",
    "style": "rounded,filled",
    "pencolor": "#FF9900",
    "penwidth": "2.5",
}

cluster_vpc_attr = {
    "fontsize": "15",
    "fontname": "Helvetica-Bold",
    "fontcolor": "#232F3E",
    "bgcolor": "#FFFFFF",
    "style": "rounded,filled",
    "pencolor": "#3F8624",
    "penwidth": "2.0",
}

cluster_eks_attr = {
    "fontsize": "14",
    "fontname": "Helvetica-Bold",
    "fontcolor": "#232F3E",
    "bgcolor": "#EDF7ED",
    "style": "rounded,dashed",
    "pencolor": "#2E7D32",
    "penwidth": "1.8",
}

with Diagram(
    "AuraFS - Distributed Local-First Cloud File System Architecture",
    filename="architecture-overview",
    show=False,
    direction="LR",
    graph_attr=graph_attr,
    node_attr=node_attr,
    edge_attr=edge_attr,
):
    user = User("Client User\n(macOS / Linux)")

    with Cluster("Local Client Host (~/aurafs)", graph_attr=cluster_client_attr):
        with Cluster("aurafs-daemon (Rust Core Engine)"):
            watcher = Rust("Watcher & Debouncer\n(notify + 500ms sliding)")
            cache = Storage("L1 Metadata Cache\n(redb pure-Rust ACID)")
            wal = Storage("Persistent WAL Queue\n(redb Write-Ahead Log)")
            sync_engine = Rust("Tokio Sync Engine\n(multipart & retry)")

            watcher >> Edge(label="stabilized event") >> wal
            wal >> Edge(label="pull queued task") >> sync_engine
            sync_engine << Edge(label="read/write state") >> cache

    user >> Edge(label="writes file to ~/aurafs") >> watcher

    with Cluster("AWS Cloud Region (eu-central-1)", graph_attr=cluster_aws_attr):
        cf = CloudFront("Amazon CloudFront\n(Edge CDN + OAC)")
        s3 = S3("Amazon S3 Raw Blobs\n(aurafs-blobs-dev)")
        sqs = SQS("Amazon SQS Event Queue\n(aurafs-events-dev + DLQ)")
        dynamo = Dynamodb("Amazon DynamoDB\n(Single-Table Metadata)")
        ecr = ECR("Amazon ECR\n(aurafs-cloud:latest)")

        with Cluster("Amazon VPC (10.0.0.0/16)", graph_attr=cluster_vpc_attr):
            with Cluster("Amazon EKS Cluster (aurafs-eks-dev)", graph_attr=cluster_eks_attr):
                irsa = IAMRole("IRSA Role\n(aurafs-cloud-pod-irsa)")
                cloud_worker = EKS("aurafs-cloud Pods\n(MIME, EXIF, Hash Verifier)")
                irsa >> Edge(style="dotted", label="authenticates") >> cloud_worker

    # Cross-boundary communication flows
    sync_engine >> Edge(color="#0073BB", label="1. Multipart S3 Upload\n(5MB chunks)") >> s3
    sync_engine >> Edge(color="#2E7D32", label="2. Conditional PutItem\n(version fencing)") >> dynamo
    cf >> Edge(color="#FF9900", label="Origin Fetch (OAC)") >> s3
    user << Edge(color="#FF9900", label="Accelerated Download") >> cf

    s3 >> Edge(color="#D13212", label="3. S3 Event Notification\n(ObjectCreated)") >> sqs
    sqs >> Edge(color="#D13212", label="4. SQS Long-Poll Batch") >> cloud_worker
    cloud_worker >> Edge(color="#0073BB", label="5. Inspect Blob Byte Range") >> s3
    cloud_worker >> Edge(color="#2E7D32", label="6. UpdateItem enriched metadata\n(MIME, tags, hash)") >> dynamo
    ecr >> Edge(style="dashed", label="pull container image") >> cloud_worker
