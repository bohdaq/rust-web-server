# syntax=docker/dockerfile:1
# rws-e2e-app: the stage end-to-end suite's test app. Build from the
# repository root (the crate depends on rws by path):
#   docker build -f ci/stage/app/Dockerfile -t rws-e2e-app:local .
# Dockerfile.dockerignore (next to this file) keeps the context small.

# The repository's MSRV (rust-version in the root Cargo.toml).
FROM rust:1.99.1-bookworm AS build
WORKDIR /src
COPY . .
# Cache mounts: the registry and the target dir survive between builds; the
# binary is copied out of the mount in the same step.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/ci/stage/app/target \
    cargo build --release --locked --manifest-path ci/stage/app/Cargo.toml \
 && mkdir -p /out \
 && cp ci/stage/app/target/release/rws-e2e-app /out/

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=build /out/rws-e2e-app /usr/local/bin/rws-e2e-app
COPY ci/stage/app/migrations /app/migrations
COPY ci/stage/app/templates /app/templates
ENV RWS_CONFIG_IP=0.0.0.0 \
    RWS_CONFIG_PORT=8080 \
    MIGRATIONS_DIR=/app/migrations
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/rws-e2e-app"]
