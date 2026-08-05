# syntax=docker/dockerfile:1

# Build the AWS/S3 binary. A full rust image (not -slim) is used because `ring`
# still needs a C compiler; the project already dropped aws-lc-rs to avoid the
# heavier CMake/NASM toolchain.
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
# Cache mounts speed up repeated local builds; on ephemeral CI runners they
# start empty (type=gha caches layers, not cache mounts). The target-dir mount
# is keyed per platform because BuildKit shares cache mounts by target path, so
# a single builder doing both platforms would have two cargos writing
# target/release/nx-cache-aws and the `cp` could grab the wrong one.
ARG TARGETPLATFORM
RUN --mount=type=cache,id=cargo-target-${TARGETPLATFORM},target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --bin nx-cache-aws && \
    cp target/release/nx-cache-aws /nx-cache-aws

# Distroless cc: glibc + ca-certificates, which rustls-native-certs reads from
# /etc/ssl/certs to trust S3's TLS roots. No shell, no package manager. The
# :nonroot tag runs as uid 65532; the server binds 3000 and writes nothing.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /nx-cache-aws /usr/local/bin/nx-cache-aws
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/nx-cache-aws"]
