## Docs

Module-level documentation for each component.

1. [Request](https://github.com/bohdaq/rust-web-server/tree/main/src/request)
1. [Header](https://github.com/bohdaq/rust-web-server/tree/main/src/header)
1. [Response](https://github.com/bohdaq/rust-web-server/tree/main/src/response)
1. [Server](https://github.com/bohdaq/rust-web-server/tree/main/src/server)
1. [Application](https://github.com/bohdaq/rust-web-server/tree/main/src/application)
1. [Controller](https://github.com/bohdaq/rust-web-server/tree/main/src/controller)
1. [TLS](https://github.com/bohdaq/rust-web-server/tree/main/src/tls) — `SniCertResolver` for SNI-based cert selection; `create_tls_acceptor_from_vhosts()` and `create_quinn_server_config_from_vhosts()` for multi-domain TLS; requires `http2` or `http3` feature
1. [Virtual Host](https://github.com/bohdaq/rust-web-server/tree/main/src/virtual_host) — `VirtualHostConfig { domain, cert_file, key_file }` — per-domain certificate configuration for virtual hosting
1. [H2 Handler](https://github.com/bohdaq/rust-web-server/tree/main/src/h2_handler) — HTTP/2 connection and stream handling; requires `http2` feature
1. [H3 Handler](https://github.com/bohdaq/rust-web-server/tree/main/src/h3_handler) — HTTP/3 over QUIC connection and stream handling; requires `http3` feature
1. [Body](https://github.com/bohdaq/rust-web-server/tree/main/src/body)
1. [JSON](https://github.com/bohdaq/rust-web-server/tree/main/src/json)
1. [URL](https://github.com/bohdaq/rust-web-server/tree/main/src/url)
1. [Null](https://github.com/bohdaq/rust-web-server/tree/main/src/null)
1. [Core](https://github.com/bohdaq/rust-web-server/tree/main/src/core)
1. [Proxy](https://github.com/bohdaq/rust-web-server/tree/main/src/proxy) — `ReverseProxy`, `H2ReverseProxy`, and `GrpcProxy` middleware with round-robin load balancing and automatic failover
1. [Rewrite](https://github.com/bohdaq/rust-web-server/tree/main/src/rewrite) — `RewriteLayer` middleware; request header/URI rewriting and response header/status/body rewriting
1. [TCP Proxy](https://github.com/bohdaq/rust-web-server/tree/main/src/tcp_proxy) — `TcpProxy` standalone L4 TCP proxy; bidirectional byte relay with round-robin backends
1. [UDP Proxy](https://github.com/bohdaq/rust-web-server/tree/main/src/udp_proxy) — `UdpProxy` standalone UDP proxy; request-reply datagram forwarding with configurable reply timeout
1. [WS Proxy](https://github.com/bohdaq/rust-web-server/tree/main/src/ws_proxy) — `WsProxy` standalone WebSocket proxy; HTTP upgrade + bidirectional frame relay
1. [Canary](https://github.com/bohdaq/rust-web-server/tree/main/src/canary) — `CanaryLayer` weighted traffic-splitting middleware; `WeightedBackend` carries url + weight
1. [Circuit Breaker](https://github.com/bohdaq/rust-web-server/tree/main/src/circuit_breaker) — `CircuitBreaker` per-backend state machine; `RetryLayer` retries on configurable status codes
1. [Service Discovery](https://github.com/bohdaq/rust-web-server/tree/main/src/service_discovery) — `BackendPool` with Static / EnvPrefix / File / Dns discovery sources; background polling
1. [Ingress](https://github.com/bohdaq/rust-web-server/tree/main/src/ingress) — `KubernetesIngressWatcher` polls K8s API; `IngressRouter` routes requests to cluster services
1. [Scheduler](https://github.com/bohdaq/rust-web-server/tree/main/src/scheduler) — `Scheduler` background task runner; fixed-rate, fixed-delay, and 6-field cron; `CronSchedule` with full field syntax
1. [Cache](https://github.com/bohdaq/rust-web-server/tree/main/src/cache) — `CacheLayer` middleware; in-memory TTL cache for GET responses with vary-by-header and capacity eviction
1. [Config Reload](https://github.com/bohdaq/rust-web-server/tree/main/src/config_reload) — hot config reload via SIGHUP; `ConfigSnapshot` exposes reloadable values; `RateLimiter` limits update live
1. [Otel](https://github.com/bohdaq/rust-web-server/tree/main/src/otel) — `OtelLayer` middleware; W3C Trace Context propagation; OTLP HTTP export to Jaeger / Grafana Tempo; `StdoutExporter` for development
1. [Template](https://github.com/bohdaq/rust-web-server/tree/main/src/template) — `TeraEngine` Jinja2/Django HTML template engine; `from_dir` / `from_glob` / `from_raw`; global singleton; `Context` re-exported from Tera; requires `tera` feature
1. [Config Binding](https://github.com/bohdaq/rust-web-server/tree/main/src/config_binding) — `FromEnvStr` trait + impls for all scalar types; `load_required` / `load_with_default` / `load_optional` helpers; used by `#[derive(Config)]` (requires `macros` feature)
