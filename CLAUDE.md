# CLAUDE.md

Guidance for AI agents working in this repository.

## Project Overview

`nx-cache-server` is a self-hosted [Nx custom remote cache](https://nx.dev/recipes/running-tasks/self-hosted-caching) server written in Rust. It implements the Nx remote-cache HTTP contract and streams build artifacts to/from cloud object storage. The only backend shipped today is **AWS S3** (and S3-compatible services like MinIO), built as the `nx-cache-aws` binary.

- Single self-contained executable, no runtime deps.
- Stack: `axum` (HTTP), `aws-sdk-s3` / `aws-config` (storage), `clap` (config), `tracing` (logging), `tokio` (runtime).
- This checkout is a fork: `origin` → `git@github.com:rodrigofariow/nx-cache-server.git`, `upstream` → `git@github.com:nxcite/nx-cache-server.git`.

## Build / Run / Test

```bash
# Build (debug)
cargo build

# Release build for the published Linux x86_64 target (matches the release workflow)
cargo build --release --bin nx-cache-aws --target x86_64-unknown-linux-gnu
# → target/x86_64-unknown-linux-gnu/release/nx-cache-aws

# Run (config via flags or env — see README)
cargo run --bin nx-cache-aws -- \
  --bucket-name my-bucket --region eu-north-1 \
  --service-access-token <token> --log-level debug

# Lint / format / test
cargo clippy --all-targets
cargo fmt
cargo test
```

No `rust-toolchain.toml` is pinned; the release CI uses `rustup update stable`. Release builds are cut **manually** via the `Release` GitHub Action (`workflow_dispatch`, input = version tag like `v1.2.0`), which cross-compiles the 5 targets in `.github/workflows/release.yml` and publishes a GitHub Release.

## Architecture

Clean three-layer split under `src/`:

- **`domain/`** — storage-agnostic core.
  - `storage.rs`: the `StorageProvider` trait (`exists` / `store` / `retrieve`) and `StorageError` (`NotFound` / `AlreadyExists` / `OperationFailed`). This is the seam any new backend implements.
  - `config.rs`: `ServerConfig` (clap), `ConfigValidator` trait, `ConfigError` (with rich user-facing messages), and `LogLevel`.
- **`infra/`** — concrete backends. `aws.rs` holds `AwsStorageConfig` (clap) + `S3Storage` implementing `StorageProvider` over `aws-sdk-s3`.
- **`server/`** — HTTP layer.
  - `mod.rs`: router + `AppState<T>` + `run_server`.
  - `handlers.rs`: the three endpoints.
  - `middleware.rs`: bearer-token auth (constant-time compare via `subtle`).
  - `validation.rs`: hash key validation (alphanumeric/`-`/`_`, ≤128 chars).
  - `error.rs`: `ServerError` → HTTP mapping.
- **`bin/aws.rs`** — wires `ServerConfig` + `AwsStorageConfig` into the `nx-cache-aws` clap CLI, initializes logging, then `run_server`.

The server is generic over `T: StorageProvider`, so adding a backend (GCS/Azure/etc.) means: a new `infra/<backend>.rs` implementing the trait, a new `src/bin/<backend>.rs`, and a `[[bin]]` entry in `Cargo.toml`.

### HTTP API (Nx contract)

| Method | Route | Auth | Behavior |
|---|---|---|---|
| `GET`  | `/health` | none | `200 "OK"` |
| `GET`  | `/v1/cache/{hash}` | Bearer | `200` + `application/octet-stream` body on hit; `404` on miss |
| `PUT`  | `/v1/cache/{hash}` | Bearer | `202` on store; `409` if the key already exists (cache entries are immutable — never overwritten) |

Auth failures return `401`. Nx clients connect via `NX_SELF_HOSTED_REMOTE_CACHE_SERVER` + `NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN` (must equal the server's `--service-access-token`).

## Logging — important

`tracing-subscriber` is compiled **without the `env-filter` feature** (`Cargo.toml`: `default-features = false, features = ["fmt"]`). Therefore:

- **`RUST_LOG` is inert.** It does nothing. Do not suggest it.
- Verbosity is controlled by `--log-level <trace|debug|info|warn|error>` / `LOG_LEVEL`, or the `--debug` shorthand (= `debug`). Resolution order lives in `bin/aws.rs`: `--log-level` wins, else `--debug`, else `info`. Implemented via `tracing_subscriber::fmt().with_max_level(...)`.

### The cache hit/miss visibility gap

The handlers (`server/handlers.rs`) and the S3 layer emit **no per-request logs on the success path** — only `tracing::error!` on failures, plus the two startup `info!` lines. So running at `--log-level=debug` and seeing "no cache hits" is expected: **a successful GET/PUT logs nothing today.** If asked to "see cache hits", the fix is to add `tracing::info!`/`debug!` lines in `retrieve_artifact` / `store_artifact` (hit, miss, store, conflict), not to change the log level. There is no request-logging middleware (e.g. `tower-http::trace`) wired in.

## Gotchas & known rough edges

- **S3 + missing `s3:ListBucket` IAM permission:** S3 returns `403 AccessDenied` (not `404 NoSuchKey`) for `GetObject`/`HeadObject` on a non-existent key when the caller lacks `s3:ListBucket`. `S3Storage` only maps `NoSuchKey`/`NotFound` → `StorageError::NotFound`; everything else → `OperationFailed` → HTTP `500`. So a cache *miss* surfaces as a 500 unless the IAM policy grants `s3:ListBucket` on the **bucket** ARN (`arn:aws:s3:::bucket`, no `/*`) in addition to `s3:GetObject`/`s3:PutObject` on the **object** ARN (`arn:aws:s3:::bucket/*`).
- **401 lacks a body/content-type:** `auth_middleware` returns a bare `Err(StatusCode::UNAUTHORIZED)` with no `text/plain` body. Nx may report *"Misconfigured remote cache endpoint: Requests should respond with text/plain on 401s."* (Note: `ServerError::Unauthorized` in `error.rs` *does* set `text/plain`, but the middleware short-circuits before that path.)
- **Not actually streaming yet:** despite the README's streaming claims, `store` buffers the whole body into a `Vec<u8>` before the S3 `put_object` (see `TODO`s in `handlers.rs` / `infra/aws.rs`). Retrieve does stream.
- **Nx never caches failed tasks.** If a wrapped task exits non-zero, Nx writes nothing to the remote cache — so "nothing in the cache" can be a failing build, not a cache bug.
- **Immutable entries:** `store` checks `exists` first and returns `409`; there is no overwrite path.

## Conventions

- Conventional Commits. End commit messages with the trailer:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
- Keep the layer boundaries: domain has no AWS/axum imports; backends depend only on the `domain` traits; HTTP concerns stay in `server/`.
- New config flags go on the relevant clap struct (`ServerConfig` or `AwsStorageConfig`) with both a `long` flag and an `env`, and should be documented in `README.md`.
