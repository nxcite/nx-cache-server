# Nx Custom Remote Cache Server

[![Release](https://github.com/nxcite/nx-cache-server/actions/workflows/release.yml/badge.svg)](https://github.com/nxcite/nx-cache-server/actions/workflows/release.yml)

A lightweight, high-performance Nx cache server that bridges Nx CLI clients with cloud storage providers for caching build artifacts. Built in Rust with a focus on maximum performance and minimal memory usage - less than 4MB during regular operation! 🚀

## Features

- **AWS S3 Integration**: Direct streaming integration with AWS S3 and S3-compatible services
- **Memory Efficient**: Direct streaming with less than 4MB RAM usage during typical operation
- **High Performance**: Built with Rust and Axum for maximum throughput
- **Zero Dependencies**: Self-contained single executable with no external dependencies required
- **Nx API Compliant**: Full implementation of the [Nx custom remote cache OpenAPI specification](https://nx.dev/recipes/running-tasks/self-hosted-caching#build-your-own-caching-server)
- **Security First**: Bearer token authentication with constant-time comparison
- **Self-Hosted & Private**: Full control over your data with zero telemetry

## Quick Start

### Prerequisites

Access to AWS S3 (or S3-compatible service like MinIO)

### Installation

#### Step 1: Download the binary
Go to [Releases page](https://github.com/nxcite/nx-cache-server/releases) and download the binary for your operating system.

Alternatively, use command line tools:
```bash
# Using curl
curl -L https://github.com/nxcite/nx-cache-server/releases/download/<VERSION>/nx-cache-aws-<VERSION>-<PLATFORM> -o nx-cache-aws

# Using wget
wget https://github.com/nxcite/nx-cache-server/releases/download/<VERSION>/nx-cache-aws-<VERSION>-<PLATFORM> -O nx-cache-aws

# Replace:
#  <VERSION> with the version tag (e.g., v1.2.0)
#  <PLATFORM> with your platform (e.g., linux-x86_64, macos-arm64, macos-x86_64, windows-x86_64.exe).
```

#### Step 2: Make executable (Linux/macOS only)
```bash
chmod +x nx-cache-aws
```

#### Step 3: Configure the server

The server supports configuration via environment variables, command-line arguments, or both.

##### Option A: Environment Variables (Recommended)
```bash
# Required
export S3_BUCKET_NAME="your-s3-bucket-name"
export SERVICE_ACCESS_TOKEN="your-bearer-token"

# AWS Credentials (optional - auto-discovered from IAM roles, config files, SSO if not provided)
export AWS_ACCESS_KEY_ID="your-aws-access-key-id"
export AWS_SECRET_ACCESS_KEY="your-aws-secret-access-key"
export AWS_SESSION_TOKEN="your-session-token"  # If you are using temporary credentials

# AWS Region (optional - auto-discovered from AWS config, EC2/ECS metadata if not provided)
export AWS_REGION="us-west-2"

# Optional
export S3_ENDPOINT_URL="your-s3-endpoint-url"   # For S3-compatible services like MinIO
export S3_TIMEOUT="30"                          # S3 operation timeout in seconds (default: 30)
export PORT="3000"                              # Server port (default: 3000)
export BIND_ADDRESS="0.0.0.0"                   # IP to bind to (default: 0.0.0.0). Use "::" for IPv6/dual-stack
export READ_ONLY_ACCESS_TOKEN="your-ro-token"   # Read-only token for untrusted CI jobs (see "Protecting against cache poisoning")
```

##### Option B: Command Line Arguments
```bash
./nx-cache-aws \
  --region "your-aws-region" \
  --access-key-id "your-aws-access-key-id" \
  --secret-access-key "your-aws-secret-access-key" \
  --bucket-name "your-s3-bucket-name" \
  --session-token "your-session-token" \
  --endpoint-url "your-s3-endpoint-url" \
  --service-access-token "your-bearer-token" \
  --timeout-seconds 30 \
  --port 3000 \
  --bind-address 0.0.0.0
```

##### Option C: Mixed Configuration
You can also combine both methods. Command line arguments will override environment variables:
```bash
# Set common config via environment
export AWS_REGION="us-west-2"
export S3_BUCKET_NAME="my-cache-bucket"
export SERVICE_ACCESS_TOKEN="my-secure-token"

# Specify other values via CLI
./nx-cache-aws --port 8080
```

> **Note:** AWS credentials and region are optional when running on AWS infrastructure (EC2, ECS, Lambda) or when AWS config files are present. The server will auto-discover them from your environment.

#### Step 4: Run the server
```bash
./nx-cache-aws
```

#### Step 5 (optional): Verify the service is up and running
```bash
curl http://localhost:3000/health
```
You should receive an "OK" response.

### Docker

Multi-arch (`linux/amd64`, `linux/arm64`) images are published to GHCR on every
release and every push to `master`. `latest` always points at the newest
release:

```bash
docker run -p 3000:3000 \
  -e S3_BUCKET_NAME="your-s3-bucket-name" \
  -e SERVICE_ACCESS_TOKEN="your-bearer-token" \
  -e AWS_REGION="us-west-2" \
  -e AWS_ACCESS_KEY_ID="your-aws-access-key-id" \
  -e AWS_SECRET_ACCESS_KEY="your-aws-secret-access-key" \
  ghcr.io/nxcite/nx-cache-server:latest
```

Tags: `latest` and `X.Y.Z` (releases), `master` (tip of the default branch),
`sha-<short-sha>` (any build).
The image is distroless — no shell, no package manager, runs as uid 65532 — and
takes the same environment variables and CLI arguments as the binary. The
container gets no AWS config files or instance metadata, so credentials have to
be passed in unless it runs somewhere the SDK can discover them (ECS/EKS task
roles, or `-v ~/.aws:/home/nonroot/.aws:ro` locally).

### Kubernetes

The server is stateless and holds no local cache, so a plain Deployment and
Service are all it needs:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nx-cache-server
spec:
  replicas: 2
  selector:
    matchLabels: { app: nx-cache-server }
  template:
    metadata:
      labels: { app: nx-cache-server }
    spec:
      containers:
        - name: server
          image: ghcr.io/nxcite/nx-cache-server:latest
          ports:
            - containerPort: 3000
          env:
            - name: S3_BUCKET_NAME
              value: your-s3-bucket-name
            - name: AWS_REGION
              value: us-west-2
            - name: SERVICE_ACCESS_TOKEN
              valueFrom:
                secretKeyRef: { name: nx-cache-server, key: service-access-token }
          livenessProbe:
            httpGet: { path: /health, port: 3000 }
          readinessProbe:
            httpGet: { path: /health, port: 3000 }
---
apiVersion: v1
kind: Service
metadata:
  name: nx-cache-server
spec:
  selector: { app: nx-cache-server }
  ports:
    - port: 80
      targetPort: 3000
```

Create the token secret with
`kubectl create secret generic nx-cache-server --from-literal=service-access-token=...`.
The manifest above passes no AWS credentials: on EKS, attach an IAM role to the
ServiceAccount (IRSA or Pod Identity) and the SDK discovers it. Elsewhere, add
`AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` from the same Secret.

### Client Configuration

To configure your Nx workspace to use this cache server, set the following environment variables:

```bash
# Point Nx to your cache server
export NX_SELF_HOSTED_REMOTE_CACHE_SERVER="http://localhost:3000"

# Authentication token (must match SERVICE_ACCESS_TOKEN from server config,
# or READ_ONLY_ACCESS_TOKEN for jobs that should not write to the cache)
export NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN="your-bearer-token"

# Optional: Disable TLS certificate validation (e.g. for development/testing environment)
export NODE_TLS_REJECT_UNAUTHORIZED="0"
```

Once configured, Nx will automatically use your cache server for storing and retrieving build artifacts.

For more details, see the [Nx documentation](https://nx.dev/recipes/running-tasks/self-hosted-caching#usage-notes).

### Protecting against cache poisoning (CVE-2025-36852 / CREEP)

If untrusted contributors can run CI with cache **write** access (typically pull request builds), they can pre-seed the cache entry for a hash that a trusted branch will later compute — and the trusted build will replay the poisoned artifact ([CVE-2025-36852, "CREEP"](https://nx.dev/blog/cve-2025-36852-critical-cache-poisoning-vulnerability-creep)). Write-once semantics don't prevent this: the attack writes *first*, it never overwrites.

The mitigation is to keep untrusted jobs read-only. Configure a second token on the server:

```bash
export SERVICE_ACCESS_TOKEN="your-rw-token"     # trusted builds (main/release): read-write
export READ_ONLY_ACCESS_TOKEN="your-ro-token"   # untrusted builds (PRs): read-only
```

Then set `NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN` to the read-only token in PR pipelines and to the read-write token only in trusted-branch pipelines. A read-only token can retrieve artifacts as usual but gets `403 Forbidden` on writes, so untrusted jobs still benefit from cache hits without being able to poison the cache.

---

### Stay Updated. Watch this repository to get notified about new releases!

<img width="369" height="387" alt="image" src="https://github.com/user-attachments/assets/97c4ebab-75a1-4f83-bc52-cf4ebbc73bfa" />

<img width="465" height="366" alt="image" src="https://github.com/user-attachments/assets/512af549-0e9a-40ac-95bd-f9eea0da38a7" />


