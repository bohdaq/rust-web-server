# rws

[![CI](https://github.com/bohdaq/rust-web-server/actions/workflows/ci.yml/badge.svg)](https://github.com/bohdaq/rust-web-server/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/rust-web-server.svg)](https://crates.io/crates/rust-web-server)
[![docs.rs](https://docs.rs/rust-web-server/badge.svg)](https://docs.rs/rust-web-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![MSRV: 1.88](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://www.rust-lang.org)

**A single-binary web server and reverse proxy, written in Rust.** Automatic HTTPS from Let's Encrypt, HTTP/3, load balancing with health checks, auth and rate limiting — configured in one TOML file, no code required. When you outgrow configuration, the same engine is a Rust library.

**Website:** [rws8.tech](https://rws8.tech/)

```bash
brew install bohdaq/tap/rws          # macOS / Linux — or: cargo binstall rust-web-server
docker run -p 7878:7878 -v "$PWD:/srv" bohdaq/rws
```

Prebuilt binaries for Linux (static, any distro), macOS, and Windows are on [GitHub Releases](https://github.com/bohdaq/rust-web-server/releases/latest).

---

## Serve a folder

```bash
cd my-site && rws
```

Serves the current directory on `http://localhost:7878` — `index.html`, directory listings, ETags and browser caching (`304` revalidation), gzip, byte ranges (video seeking, resumable downloads), and an optional SPA fallback for client-side routers. `rws --help` lists every option.

## Automatic HTTPS

```toml
# rws.config.toml
port               = 443
http_redirect_port = 80

[acme]
domains = ["example.com", "www.example.com"]
email   = "you@example.com"
```

`rws` gets certificates from Let's Encrypt on first start, serves HTTPS with HTTP/2 and HTTP/3, redirects HTTP to HTTPS, and renews in the background with zero-downtime reloads. Retries until DNS is ready, IPv4 and IPv6, private ACME CAs supported. → [Automatic HTTPS](https://rws8.tech/features/acme/)

## Reverse proxy

```toml
[[upstream]]
name     = "app"
backends = ["10.0.0.10:8080", "10.0.0.11:8080"]
strategy = "least_connections"

  [upstream.health_check]
  path = "/healthz"

[[route]]
  [route.match]
  host = "api.example.com"
  path = "/v1/*"

  [route.action]
  type     = "proxy"
  upstream = "app"

  [route.middleware]
  rate_limit = { max_requests = 500, window_secs = 60 }
  auth       = { type = "bearer", token_env = "API_TOKEN" }
```

Load balancing (round-robin, least-connections, random, IP hash), active health checks, bearer / JWT / basic auth, rate limits, caching, IP allow/deny lists, header and URL rewrites, per-route timeouts and body limits — plus TCP, UDP, WebSocket, and gRPC proxying. Misconfigured auth fails closed. Circuit breakers, canary splits, forward auth, and service discovery are available from the library. Combine with `[acme]` for HTTPS in front of your apps. → [Config-driven proxy](https://rws8.tech/proxy/config-driven/) · [full config reference](https://rws8.tech/configuration/config-file/)

---

## Need custom logic? It's also a Rust library

The server above is the `rust-web-server` crate. Use it directly when you need routes, middleware, or an MCP server in code:

```toml
# Cargo.toml
[dependencies]
rust-web-server = "17"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust,no_run
use rust_web_server::prelude::*;

fn hello(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
    Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "Hello, world!")
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

Routing with path parameters, typed extractors, one `Middleware` trait for everything (auth, rate limiting, caching, tracing, the reverse proxy itself), WebSocket, SSE, an MCP server, an async ORM, background jobs, object storage, a mailer, and OAuth/OIDC/SAML login — [all features](https://rws8.tech/getting-started/features/), [building apps](https://rws8.tech/building-apps/overview/).

## Why rws

- **One binary, one file.** Static site, HTTPS, reverse proxy, and gateway features in a single executable configured by one `rws.config.toml` — no separate certbot, cron job, or proxy process.
- **No hyper, no Tower, no Axum.** HTTP/1.1 parsing, routing, middleware, WebSocket, SSE, CORS, range requests, JWT, and the ACME client are implemented in this crate. HTTP/2 and HTTP/3 framing use the widely deployed `h2` and `quinn`/`h3` crates, and TLS uses `rustls` — no OpenSSL.
- **Config and code share one engine.** Everything the TOML file does is a type you can use from Rust, so moving from configuration to code doesn't mean switching tools.
- **Tested against the real world.** CI fuzzes every parser that sees untrusted input, checks HTTP/2 conformance with h2spec, and runs the ACME flow end to end against Let's Encrypt's test CA on every push. Report vulnerabilities privately — see [SECURITY.md](SECURITY.md).
- **Fast.** 2.5–5.1× faster than Caddy, and less CPU per request than nginx for static files, proxying, HTTPS and access-logged traffic — see [Performance](#performance).
- **SemVer.** Breaking changes only in a major version — see the [CHANGELOG](CHANGELOG.md).

## Performance

Requests per second (p99 latency), on one 4-CPU GitHub Actions runner with
the load generator on the same machine: `oha`, 64 keep-alive connections,
20 s per scenario, no compression, access logs off; nginx with
`open_file_cache`.

| Scenario | rws | Caddy 2.11 | nginx |
|---|---:|---:|---:|
| Static file, 1 KiB | **95,728** (**1.6 ms**) | 18,897 (13.1 ms) | 82,218 (3.0 ms) |
| Static file, 1 MiB | 9,508 (**14.4 ms**) | 2,419 (124 ms) | **9,874** (14.5 ms) |
| Reverse proxy, 1 KiB | **34,732** (**3.8 ms**) | 13,774 (13.8 ms) | 34,424 (4.4 ms) |
| CPU per request (1 KiB / 1 MiB / proxy) | **16.6** / **211** / **44.4** µs | 151 / 1,177 / 187 µs | 25.0 / 213 / 46.6 µs |
| Peak memory | 34–38 MiB | 68–72 MiB | **23–24 MiB** |

`rws` is 2.5–5.1× faster than Caddy and has the lowest p99 of the three in
every scenario. It needs less CPU per request than nginx for small files
(−34%), large files (−1%), proxying (−5%), access-logged traffic (−27%),
HTTPS (−23%) and gzip (a hundredth); nginx is ahead over HTTP/2 (+23%) and,
slightly, for large files over TLS (+6%). Release builds use the mimalloc
allocator: about 5% less CPU, but more memory than nginx (build without the
`mimalloc` feature for ~9 MiB) — see the
[benchmarks page](https://rws8.tech/reference/benchmarks/). Shared CI runners are noisy, so
compare servers within a run, not across runs. Reproduce it with
[`bench/run.sh`](bench/run.sh) or the manual
[Benchmark workflow](.github/workflows/bench.yml).

## Documentation

Everything is on **[rws8.tech](https://rws8.tech/)** (source: [`docs/`](docs/src/content/docs/)):

- [Quick start](https://rws8.tech/getting-started/quick-start/) · [Installation and feature flags](https://rws8.tech/getting-started/installation/) · [All features](https://rws8.tech/getting-started/features/)
- [Configuration](https://rws8.tech/configuration/overview/) — environment variables, `rws.config.toml`, CLI flags
- [Building apps](https://rws8.tech/building-apps/overview/) — routing, state, extractors, middleware, templates, the ORM
- [Proxy](https://rws8.tech/proxy/overview/) · [MCP server](https://rws8.tech/mcp/overview/) · [Deployment](https://rws8.tech/deployment/docker/)
- [Architecture](https://rws8.tech/reference/architecture/) · [Troubleshooting](https://rws8.tech/reference/troubleshooting/) · [Migrating to 18.0](https://rws8.tech/reference/migrating-18/)
- API reference: [docs.rs/rust-web-server](https://docs.rs/rust-web-server) · Changes: [CHANGELOG.md](CHANGELOG.md)

## Building apps with AI

`rws` is easy for coding assistants to get right: one routing pattern, one
`Middleware` trait for everything, and no hyper/Tower/Axum types to confuse
with this crate's own. Point your assistant at:

- **[llms.txt](llms.txt)** — an index of the docs with a one-line summary per page;
  raw: `https://raw.githubusercontent.com/bohdaq/rust-web-server/main/llms.txt`
- **[llms-full.txt](llms-full.txt)** — every docs page in one file, for pasting into a chat or system prompt
- **[CLAUDE.md](CLAUDE.md)** — architecture and conventions; Claude Code reads it automatically inside this repo

```text
I'm building on the rust-web-server (rws) crate. Read
https://raw.githubusercontent.com/bohdaq/rust-web-server/main/llms-full.txt
then build a REST API with GET/POST /todos backed by SQLite (model-sqlite),
JWT auth on POST (auth), and per-IP rate limiting on every route.
```

To make your app a tool that Claude, Cursor and other MCP clients can call,
see the [MCP server](https://rws8.tech/mcp/overview/).

## Build from source

```bash
git clone https://github.com/bohdaq/rust-web-server.git
cd rust-web-server
cargo build --release      # target/release/rws; MSRV 1.88
```

`--no-default-features --features http2` drops HTTP/3; `--features http1`
builds a plain HTTP/1.1 server with no TLS and no async runtime. Contributor
guide: [DEVELOPER.md](DEVELOPER.md).

## Contributing

Issues labelled [good first issue](https://github.com/bohdaq/rust-web-server/labels/good%20first%20issue) are a good place to start; questions go to [Discussions](https://github.com/bohdaq/rust-web-server/discussions). See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build, test and send a change.

## License

MIT
