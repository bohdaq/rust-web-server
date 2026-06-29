# ── build stage ──────────────────────────────────────────────────────────────
FROM rust:1.75-slim AS builder

# aws-lc-rs (used by rustls) needs cmake, clang, and a C linker
RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake clang pkg-config && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

# ── runtime stage ─────────────────────────────────────────────────────────────
# gcr.io/distroless/cc includes only the C runtime libraries needed for
# statically-linked Rust + aws-lc-rs. No shell, no package manager.
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/target/release/rws /usr/local/bin/rws

# Default port — override at runtime with RWS_CONFIG_PORT
EXPOSE 7878

# The server binds 0.0.0.0 by default so the K8s Service can reach it.
# Pass TLS paths via env vars or volume mounts:
#   -e RWS_CONFIG_TLS_CERT_FILE=/certs/tls.crt
#   -e RWS_CONFIG_TLS_KEY_FILE=/certs/tls.key
CMD ["/usr/local/bin/rws"]
