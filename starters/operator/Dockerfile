# The operator: a release build in Rust's image, run from distroless as
# nonroot. `docker build -t <image> .`, then
# `docker run --rm <image> --manifests --image <image> | kubectl apply -f -`.
FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release && cp target/release/operator /usr/local/bin/operator

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /usr/local/bin/operator /operator
ENTRYPOINT ["/operator"]
