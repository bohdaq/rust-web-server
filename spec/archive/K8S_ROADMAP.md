# Kubernetes Readiness Roadmap

## Critical — server won't work in a pod without these

### ✅ 1. Bind to `0.0.0.0` instead of `127.0.0.1`

Default IP in `src/entry_point/mod.rs` (`RWS_CONFIG_IP_DEFAULT_VALUE`) is `127.0.0.1`. Inside a container that means only loopback — the K8s Service and health probes talk to the pod IP, so they get refused. Change the default to `0.0.0.0`.

### ✅ 2. Health check endpoints (`/healthz` and `/readyz`)

K8s `livenessProbe` and `readinessProbe` hit an HTTP path. The server has no such controllers today. Without them you cannot set meaningful probes, so K8s cannot restart stuck pods or hold traffic during startup.

Add two controllers:
- `GET /healthz` — liveness: returns `200 OK` if the process is alive
- `GET /readyz` — readiness: returns `200 OK` when the server is ready to serve traffic, `503` during startup or drain

---

## Important — operational reliability

### ✅ 3. Graceful shutdown on SIGTERM

K8s sends SIGTERM before killing a pod (rolling deploy, scale-down, node eviction). The thread-pool server (`src/server/mod.rs`) and the tokio async path have no signal handler. Without one, in-flight requests are dropped.

Required behaviour:
1. Catch SIGTERM
2. Stop accepting new connections
3. Drain the thread pool (finish in-flight requests)
4. Exit cleanly

For the `http1` feature: install a signal handler via `std::os::unix` and set a shared atomic flag the accept loop checks. For `http2`/`http3`: use `tokio::signal::unix::signal(SignalKind::terminate())` and `tokio::select!` against the accept future.

### ✅ 4. Dockerfile (multi-stage build)

Nothing exists to produce a container image. A standard multi-stage build is needed before any K8s deployment.

```dockerfile
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/rws /usr/local/bin/rws
EXPOSE 7878
CMD ["rws"]
```

Use `gcr.io/distroless/cc` in the final stage for a smaller, non-shell image in production.

---

## Nice to have — observability

### ✅ 5. Structured logging to stdout

Current `println!`/`eprintln!` produce plain text. K8s log aggregators (Loki, Fluentd, Datadog) work better with JSON. Logs already go to stdout/stderr (correct), but a JSON format option for production reduces parsing friction.

Set `RWS_CONFIG_LOG_FORMAT=json` to emit structured JSON access logs.

### ✅ 6. Prometheus `/metrics` endpoint

Without metrics, pods are observable only through logs. A `GET /metrics` endpoint in Prometheus exposition format enables:
- Request rate and error rate counters
- Active connection gauge
- Response latency histograms
- Horizontal Pod Autoscaler (HPA) integration via custom metrics

---

## Already good — no changes needed

| What | Why it works |
|---|---|
| Config via `RWS_CONFIG_*` env vars | Maps directly to K8s ConfigMaps and Secrets |
| TLS cert/key as file paths | Works with cert-manager volume mounts |
| Logs to stdout/stderr | K8s log collection expects this |
| Stateless request handling | Pods can be freely scheduled and replaced |

---

## Suggested implementation order

1. Change default bind IP to `0.0.0.0` (one-line change in `src/entry_point/mod.rs`)
2. Add `/healthz` and `/readyz` controllers
3. Write a Dockerfile
4. Add SIGTERM graceful shutdown
5. Structured logging
6. Prometheus `/metrics` endpoint
