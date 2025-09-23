# Nx Custom Remote Cache Server

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
#  <VERSION> with the version tag (e.g., v1.0.0)
#  <PLATFORM> with your platform (e.g., linux-x86_64, macos-arm64, macos-x86_64, windows-x86_64.exe).
```

#### Step 2: Make executable (Linux/macOS only)
```bash
chmod +x nx-cache-aws
```

#### Step 3: Configure the server

The server supports two configuration methods that can be used independently or combined:

##### Option A: Environment Variables (Recommended)
```bash
export AWS_REGION="your-aws-region"
export AWS_ACCESS_KEY_ID="your-aws-access-key-id"
export AWS_SECRET_ACCESS_KEY="your-aws-secret-access-key"
export S3_BUCKET_NAME="your-s3-bucket-name"
export SERVICE_ACCESS_TOKEN="your-bearer-token"

# Optional:
export S3_ENDPOINT_URL="your-s3-endpoint-url"   # for S3-compatible services like MinIO
export S3_TIMEOUT="30"                          # S3 operation timeout in seconds (default: 30)
export PORT="3000"                              # server port (default: 3000)
```

##### Option B: Command Line Arguments
```bash
./nx-cache-aws \
  --region "your-aws-region" \
  --access-key-id "your-aws-access-key-id" \
  --secret-access-key "your-aws-secret-access-key" \
  --bucket-name "your-s3-bucket-name" \
  --endpoint-url "your-s3-endpoint-url" \
  --service-access-token "your-bearer-token" \
  --timeout-seconds 30 \
  --port 3000
```

##### Option C: Mixed Configuration
You can also combine both methods. Command line arguments will override environment variables:
```bash
# Set common config via environment
export AWS_REGION="us-west-2"
export AWS_ACCESS_KEY_ID="your-aws-access-key-id"
export AWS_SECRET_ACCESS_KEY="your-aws-secret-access-key"
export S3_BUCKET_NAME="my-cache-bucket"
export SERVICE_ACCESS_TOKEN="my-secure-token"

# Override other values via CLI
./nx-cache-aws --port 8080
```

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

# Authentication token (must match SERVICE_ACCESS_TOKEN from server config)
export NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN="your-bearer-token"

# Optional: Disable TLS certificate validation (e.g. for development/testing environment)
export NODE_TLS_REJECT_UNAUTHORIZED="0"
```

Once configured, Nx will automatically use your cache server for storing and retrieving build artifacts.

For more details, see the [Nx documentation](https://nx.dev/recipes/running-tasks/self-hosted-caching#usage-notes).

---

### Stay Updated. Watch this repository to get notified about new releases!

<img width="369" height="387" alt="image" src="https://github.com/user-attachments/assets/97c4ebab-75a1-4f83-bc52-cf4ebbc73bfa" />

<img width="465" height="366" alt="image" src="https://github.com/user-attachments/assets/512af549-0e9a-40ac-95bd-f9eea0da38a7" />


