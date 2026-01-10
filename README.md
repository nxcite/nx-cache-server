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
#  <VERSION> with the version tag (e.g., v1.1.0)
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

# Access token(s) - supports single or multiple, plain or named
export SERVICE_ACCESS_TOKEN="frontend=token1,backend=token2,ci=token3"

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
  --service-access-token "frontend=token1,backend=token2" \
  --timeout-seconds 30 \
  --port 3000

# Single token also works:
# --service-access-token "my-single-token"
```

##### Option C: Mixed Configuration
You can also combine both methods. Command line arguments will override environment variables:
```bash
# Set common config via environment
export AWS_REGION="us-west-2"
export S3_BUCKET_NAME="my-cache-bucket"

# Works for single or multiple tokens
export SERVICE_ACCESS_TOKEN="my-token"  # Single token
# OR: export SERVICE_ACCESS_TOKEN="frontend=token1,backend=token2"  # Multiple tokens

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

### Client Configuration

To configure your Nx workspace to use this cache server, set the following environment variables:

```bash
# Point Nx to your cache server
export NX_SELF_HOSTED_REMOTE_CACHE_SERVER="http://localhost:3000"

# Authentication token (must match one of the tokens from SERVICE_ACCESS_TOKENS on the server)
export NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN="token1"

# Optional: Disable TLS certificate validation (e.g. for development/testing environment)
export NODE_TLS_REJECT_UNAUTHORIZED="0"
```

Once configured, Nx will automatically use your cache server for storing and retrieving build artifacts.

#### Token Configuration

**Use `SERVICE_ACCESS_TOKEN` for single OR multiple tokens:**

```bash
# Single token (plain)
export SERVICE_ACCESS_TOKEN="my-token-123"

# Single token (named for better logging)
export SERVICE_ACCESS_TOKEN="production=my-token-123"

# Multiple plain tokens (comma-separated)
export SERVICE_ACCESS_TOKEN="token1,token2,token3"

# Multiple named tokens (recommended for teams)
export SERVICE_ACCESS_TOKEN="frontend=abc123,backend=def456,ci=xyz789"

# Mixed format (named + plain)
export SERVICE_ACCESS_TOKEN="frontend=abc123,def456,ci=xyz789"
```

For more details, see the [Nx documentation](https://nx.dev/recipes/running-tasks/self-hosted-caching#usage-notes).

---

### Stay Updated. Watch this repository to get notified about new releases!

<img width="369" height="387" alt="image" src="https://github.com/user-attachments/assets/97c4ebab-75a1-4f83-bc52-cf4ebbc73bfa" />

<img width="465" height="366" alt="image" src="https://github.com/user-attachments/assets/512af549-0e9a-40ac-95bd-f9eea0da38a7" />


