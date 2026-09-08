# Working on nx-cache-server

A small Rust server that puts an S3 bucket behind the Nx remote cache HTTP
API. Most of what matters here is decided by the Nx client, not by this repo.

## The Nx client is the spec

Nx accepts a fixed set of status codes and content types and fails the build
on anything else. The rules live in
`packages/nx/src/native/cache/http_remote_cache.rs` and
`packages/nx/src/tasks-runner/cache.ts` in the Nx repo. Before changing a
status code, a content type, or when a response is sent relative to the
request body, read those files for the Nx version range we support and say
in the PR what Nx does with the new behaviour.

## Comments

- Say a thing once, then point back to it.
- If the code right below answers it, delete the comment.
- Keep what cannot be learned from this repo: what the Nx client does with a
  status, what the S3 SDK does with an error. Cite the source file.
- Before keeping a comment, delete it and check whether anything is lost.

## Before you push

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Router behaviour is tested in `src/server/mod.rs` with `tower::ServiceExt::oneshot`
against in-memory storage mocks. Add a case there rather than a new harness.

## Pull requests

One concern per PR. The description states the problem as Nx sees it, the
change, and what a user of the server will observe differently.
