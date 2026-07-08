---
title: Quick Start
description: Three paths to a running rust-web-server in under five minutes — static files, a first route, and a config-driven proxy.
---

Pick the path that matches your goal. All three end with a working server you can hit with `curl`.

---

## Path 1 — Static file server (zero code)

Install the binary and point it at a directory. No config file, no code.

```bash
cargo install rust-web-server
# create something to serve
mkdir www && echo "<h1>Hello</h1>" > www/index.html
cd www
rws
```

```
Listening on 0.0.0.0:7878
```

```bash
curl http://localhost:7878/
# <h1>Hello</h1>
```

`rws` serves every file under the current directory, handles range requests, sets ETags, negotiates gzip, and returns `404` for missing files — all with no configuration. A directory with no `index.html` renders a directory listing page instead of `404` — try `mkdir www/uploads && touch www/uploads/report.pdf && curl http://localhost:7878/uploads/`.

:::note[HTTPS in one flag]
If you have a TLS certificate, pass `--tls-cert-file=cert.pem --tls-key-file=key.pem` and the server automatically upgrades to HTTP/2 + HTTP/3:

```bash
rws --tls-cert-file=cert.pem --tls-key-file=key.pem
```
:::

---

## Path 2 — First route (library)

Use `rust-web-server` as a Rust library crate and define your own handlers.

### 1. Add dependencies

```toml title="Cargo.toml"
[dependencies]
rust-web-server = "17"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### 2. Write a handler

```rust title="src/main.rs"
use rust_web_server::prelude::*;

fn hello(_req: &Request, _params: &PathParams, _conn: &ConnectionInfo, _state: &()) -> Response {
    Response::get_response(
        STATUS_CODE_REASON_PHRASE.n200_ok,
        None,
        Some(vec![Range::get_content_range(
            b"Hello, world!".to_vec(),
            MimeType::TEXT_PLAIN.to_string(),
        )]),
    )
}

#[tokio::main]
async fn main() {
    let app = routes! {
        App::with_state(()),
        GET "/hello" => hello,
    };
    let (listener, pool) = Server::setup().unwrap();
    tokio::join!(
        Server::run_tls(listener, pool, app.clone()),
        Server::run_quic(app),
        Server::run_redirect(),
    );
}
```

### 3. Run and verify

```bash
cargo run
```

```bash
curl http://localhost:7878/hello
# Hello, world!
```

### Handler signature

Every route handler receives four arguments:

| Argument | Type | What it contains |
|---|---|---|
| `_req` | `&Request` | Method, URI, headers, raw body bytes |
| `_params` | `&PathParams` | Named path segments (e.g. `:id`) |
| `_conn` | `&ConnectionInfo` | Client/server IP, port, SNI hostname |
| `_state` | `&S` | Your shared application state |

### Shared state

Replace `()` with any `Send + Sync` type to share state across handlers:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

struct AppState {
    counter: AtomicU64,
}

fn count(_: &Request, _: &PathParams, _: &ConnectionInfo, state: &Arc<AppState>) -> Response {
    let n = state.counter.fetch_add(1, Ordering::Relaxed);
    Response::get_response(
        STATUS_CODE_REASON_PHRASE.n200_ok,
        None,
        Some(vec![Range::get_content_range(
            format!("count: {n}").into_bytes(),
            MimeType::TEXT_PLAIN.to_string(),
        )]),
    )
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState { counter: AtomicU64::new(0) });
    let app = routes! {
        App::with_state(state),
        GET "/count" => count,
    };
    let (listener, pool) = Server::setup().unwrap();
    tokio::join!(
        Server::run_tls(listener, pool, app.clone()),
        Server::run_quic(app),
        Server::run_redirect(),
    );
}
```

---

## Path 3 — Config-driven proxy (no code)

Drop an `rws.config.toml` next to the `rws` binary and it becomes a reverse proxy and load balancer. No code required.

### Minimal config

```toml title="rws.config.toml"
[server]
port = 8080

[[upstream]]
name     = "api"
backends = ["localhost:3000", "localhost:3001"]

[upstream.health_check]
path              = "/healthz"
interval_secs     = 10
healthy_threshold = 2

[[route]]
[route.match]
path = "/api/"

[route.action]
type     = "proxy"
upstream = "api"

[[route]]
[route.match]
path = "/"

[route.action]
type   = "respond"
status = 200
body   = "Gateway ready"
```

```bash
rws
# Listening on 0.0.0.0:8080
```

```bash
curl http://localhost:8080/
# Gateway ready

curl http://localhost:8080/api/users
# proxied to localhost:3000 or localhost:3001 (round-robin, health-checked)
```

### What the config does

- `[[upstream]]` declares a named backend pool with health checking. Dead backends are removed automatically; they re-enter rotation once they pass `healthy_threshold` consecutive checks.
- `[[route]]` rules are evaluated top-to-bottom, first match wins.
- `type = "proxy"` forwards the request to the named upstream. `type = "respond"` returns a fixed response.

:::note[Add middleware per route]
Rate limiting, Bearer auth, and request rewriting can all be added per-route inside the config file — no code change needed. See the Proxy & Gateway documentation for the full schema.
:::

:::caution[HTTPS in proxy mode]
Pass `--tls-cert-file` and `--tls-key-file` (or set `RWS_CONFIG_TLS_CERT_FILE` / `RWS_CONFIG_TLS_KEY_FILE`) to enable TLS in proxy mode. Virtual-host routing with per-domain certificates is supported via `[[virtual_host]]` sections.
:::

---

## Next steps

- **[Dev Workflow](/getting-started/dev-workflow/)** — auto-rebuild and restart on every code change with `cargo-watch`
- **Routing** — named path parameters (`:id`), wildcards, virtual-host routing
- **Middleware** — `RateLimitLayer`, `JwtLayer`, `CacheLayer`, `OtelLayer`, and more via `.wrap(layer)`
- **MCP Server** — expose tools and resources to AI agents over the MCP Streamable HTTP protocol
- **ORM** — `#[derive(Model)]`, `QueryBuilder`, migrations, `HasMany` / `HasOne` / `BelongsTo`
