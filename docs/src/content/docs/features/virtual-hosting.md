---
title: Virtual Hosting
description: Serve multiple domains from a single server instance, each with its own TLS certificate, using SNI-based routing.
---

## Overview

A single `rust-web-server` instance can host multiple domains simultaneously.
At the TLS handshake, the client sends its target hostname via SNI (Server Name
Indication). `SniCertResolver` reads that hostname and selects the matching
certificate before any HTTP traffic is exchanged. The negotiated hostname is
then available in every request handler as `ConnectionInfo::sni_hostname`.

Virtual hosting requires the `http2` or `http3` (default) feature for TLS
support.

## Configuring virtual hosts in `rws.config.toml`

Add one `[[virtual_host]]` block per domain. The top-level `tls_cert_file` /
`tls_key_file` pair is used as a fallback when no SNI hostname matches (or when
the client sends no SNI):

```toml
# Default certificate — used when no virtual host matches
tls_cert_file = "/etc/ssl/default.pem"
tls_key_file  = "/etc/ssl/default.key"

[[virtual_host]]
domain    = "example.com"
cert_file = "/etc/ssl/example.pem"
key_file  = "/etc/ssl/example.key"

[[virtual_host]]
domain    = "api.example.com"
cert_file = "/etc/ssl/api-example.pem"
key_file  = "/etc/ssl/api-example.key"
```

## Configuring virtual hosts via environment variables

The same configuration is available through numbered environment variables,
which is convenient in container environments:

```bash
RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN=example.com
RWS_CONFIG_VIRTUAL_HOST_0_CERT_FILE=/etc/ssl/example.pem
RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE=/etc/ssl/example.key

RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN=api.example.com
RWS_CONFIG_VIRTUAL_HOST_1_CERT_FILE=/etc/ssl/api-example.pem
RWS_CONFIG_VIRTUAL_HOST_1_KEY_FILE=/etc/ssl/api-example.key
```

## How SNI resolution works

`SniCertResolver` implements `rustls::server::ResolvesServerCert`. It holds a
`HashMap<String, Arc<CertifiedKey>>` keyed by the exact SNI hostname, plus an
optional default. The resolver is built once at startup (or after SIGHUP) by
`create_tls_acceptor_from_vhosts()`:

```rust
// src/tls/mod.rs (simplified)
pub fn create_tls_acceptor_from_vhosts(
    vhosts: &[VirtualHostConfig],
    default_cert: &str,
    default_key: &str,
) -> Result<TlsAcceptor, String>
```

The same function is used for both HTTP/2 (TCP/TLS) and HTTP/3 (QUIC) listeners,
so virtual hosting works transparently across all protocols.

## Reading the SNI hostname in handlers

After the TLS handshake, `ConnectionInfo::sni_hostname` carries the negotiated
hostname as `Option<String>`. For plain HTTP/1.1 connections (no TLS) this
field is `None` and the `Host` header should be used instead.

```rust
fn process(&self, request: &Request, response: Response, connection: &ConnectionInfo) -> Response {
    match &connection.sni_hostname {
        Some(host) => println!("Serving request for: {}", host),
        None       => println!("No SNI — plain HTTP or no matching vhost"),
    }
    response
}
```

## Host-restricted routing with `Router`

Call `.with_host("hostname")` before registering routes to restrict a `Router`
to requests whose SNI hostname (TLS) or `Host` header (plain HTTP) matches:

```rust
use rust_web_server::router::Router;

let mut api_router = Router::new();
api_router.with_host("api.example.com")
    .get("/v1/users", |_req, _params, _conn| { /* ... */ });

let mut www_router = Router::new();
www_router.with_host("example.com")
    .get("/", |_req, _params, _conn| { /* ... */ });
```

## Hot reload

Send `SIGHUP` (or `POST /admin/config/reload`) to hot-reload all virtual host
certificates from disk without restarting the server:

```bash
kill -HUP $(pidof rws)
```

`Server::run_tls` rebuilds `TlsAcceptor` with updated certificates for all
virtual hosts on every SIGHUP. New connections immediately use the refreshed
certificates; existing connections are not interrupted.
