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
export LOG_FORMAT="text"                        # Log format: text or json (default: text). See "Logging"
export DEBUG="false"                            # Debug logs for this server (default: false). See "Logging"
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

### Logging

The server logs to stdout, one line per request (routine requests at `debug`).

```bash
export LOG_FORMAT="json"   # text (default) or json
export RUST_LOG="info"     # e.g. "warn" or "info,nx_cache_server=debug"
export DEBUG="true"        # debug logs for this server; ignored when RUST_LOG is set
```

- `RUST_LOG` is a comma-separated list of `level` or `target=level` entries. Full `env_filter` syntax (span filters, regexes) is not supported. Unset or empty means `info`, or the `DEBUG` filter when `DEBUG` is set. An invalid value stops the server at startup.
- A level below `info` for this server (e.g. `RUST_LOG=warn`) also hides the `Server running on` line. An unknown word is read as a target name, so a typo like `RUST_LOG=warning` silences all output.
- `DEBUG=true` (also `1`, `yes`, `on`) logs this server at `debug` and everything else at `info`. If `RUST_LOG` is also set, `RUST_LOG` wins and the server logs a warning.
- Tokens, the `Authorization` header and request bodies are never logged.
- Startup failures print to stderr. With `LOG_FORMAT=json` they are also logged as an `error` line.

Each request logs a line with the message `request` and these fields:

| Field | Meaning |
|---|---|
| `method` | HTTP method |
| `hash` | Cache hash; omitted when missing or invalid |
| `status` | Response status |
| `outcome` | What happened (see below) |
| `access` | `read_write`, `read_only`, or `none` (no valid token) |
| `bytes` | Size of the artifact stored or retrieved; `0` otherwise |
| `duration_ms` | Time until the response headers are sent. A download streams afterwards, so for GET this is roughly S3's time to first byte; for PUT it includes the full upload |

| `outcome` | Meaning |
|---|---|
| `hit` | Artifact found (200) |
| `miss` | Artifact not found (404) |
| `stored` | Artifact uploaded (200) |
| `exists` | Upload refused, hash already stored (409) |
| `forbidden` | Read-only token tried to upload (403) |
| `unauthorized` | Missing or unknown token (401) |
| `invalid` | Bad hash or other rejected request (4xx) |
| `unmatched` | No route for the path or method (404, 405) |
| `error` | Storage failure (5xx) |
| `health` | `GET /health` |
| `aborted` | Client disconnected before the response (no `status`), or the upload broke off (400) |

Each request line's level depends on its outcome: `error` for storage failures, `warn` for `unauthorized` and `invalid`, `info` for `aborted`, and `debug` for routine traffic (`hit`, `miss`, `stored`, `exists`, `forbidden`, `health`, `unmatched`). Set `DEBUG=true` to log every request. A storage failure also logs an S3 `warn` line with `operation`, `error` and `code`, tagged with the request's hash.

Text format:
```
2026-10-21T01:47:50.548812Z DEBUG nx_cache_server::server::logging: request method=GET hash="14cdc9c4a8af4987cce2" status=200 outcome="hit" access="read_only" bytes=1048576 duration_ms=5
```

JSON format:
```json
{"timestamp":"2026-10-21T01:47:51.163412Z","level":"DEBUG","message":"request","method":"GET","hash":"932ad3fd6176a673686b","status":200,"outcome":"hit","access":"read_only","bytes":1048576,"duration_ms":2,"target":"nx_cache_server::server::logging"}
```

---

### Stay Updated. Watch this repository to get notified about new releases!

<img width="369" height="387" alt="image" src="https://github.com/user-attachments/assets/97c4ebab-75a1-4f83-bc52-cf4ebbc73bfa" />

<img width="465" height="366" alt="image" src="https://github.com/user-attachments/assets/512af549-0e9a-40ac-95bd-f9eea0da38a7" />


