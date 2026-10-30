# Frontend: lit + Vite → frontend/dist
FROM node:24.11.1-bookworm-slim AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# API: a release build of the rws app
FROM rust:1.99-bookworm AS api
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release && cp target/release/app /usr/local/bin/app

# Runtime: distroless, nonroot, no shell. /app/public is the static root
# (rws serves its working directory, which main() sets to STATIC_DIR).
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=api /usr/local/bin/app /app/app
COPY migrations /app/migrations
COPY --from=frontend /src/frontend/dist /app/public
ENV STATIC_DIR=/app/public \
    RWS_CONFIG_IP=0.0.0.0 \
    RWS_CONFIG_PORT=7878 \
    RWS_CONFIG_SPA_FALLBACK=index.html \
    RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES=/api,/healthz
EXPOSE 7878
ENTRYPOINT ["/app/app"]
