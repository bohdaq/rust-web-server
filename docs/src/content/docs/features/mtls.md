---
title: Mutual TLS (mTLS)
description: Require clients to present a certificate signed by a trusted CA on every HTTPS and QUIC connection.
---

## Overview

Mutual TLS (mTLS) enforces two-way authentication: the server presents its
certificate to the client as usual, and the client must also present a
certificate signed by a CA that the server trusts. Connections that do not
present a valid client certificate are rejected at the TLS handshake layer,
before any HTTP traffic is exchanged.

## Enabling mTLS

Set `RWS_CONFIG_TLS_CLIENT_CA_FILE` to the path of a PEM file containing the
CA certificate (or certificate chain) whose signatures you trust:

```bash
export RWS_CONFIG_TLS_CLIENT_CA_FILE=/etc/ssl/client-ca.pem
```

```toml
# rws.config.toml
tls_client_ca_file = "/etc/ssl/client-ca.pem"
```

No other configuration is needed. When this variable is set,
`create_tls_acceptor_from_vhosts()` builds a `WebPkiClientVerifier` and
attaches it to the `rustls::ServerConfig`. The same CA applies to both the
HTTPS (TCP/TLS) listener and the QUIC listener used for HTTP/3.

## How the verifier is built

```rust
// src/tls/mod.rs (simplified)
let verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
    .build()
    .unwrap();

ServerConfig::builder()
    .with_client_cert_verifier(verifier)
    .with_cert_resolver(Arc::new(resolver))
```

The CA certificate is loaded into a `RootCertStore`. Every TLS `ClientHello`
that follows must include a certificate chain rooted in that store.

## Generating test client certificates

Create a self-signed CA and a client certificate signed by it:

```bash
# 1. Generate a CA key and self-signed certificate
openssl req -x509 -newkey rsa:4096 -keyout ca-key.pem -out ca.pem \
  -days 3650 -nodes -subj "/CN=MyTestCA"

# 2. Generate a client key and certificate signing request
openssl req -newkey rsa:2048 -keyout client-key.pem -out client.csr \
  -nodes -subj "/CN=test-client"

# 3. Sign the client certificate with the CA
openssl x509 -req -in client.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out client.pem -days 365
```

## Testing with curl

```bash
curl --cert client.pem --key client-key.pem \
     --cacert cert.pem \
     https://localhost:7878/healthz
```

`--cacert cert.pem` tells curl to trust the server's self-signed certificate.
`--cert` / `--key` provide the client certificate and key that the server
validates against `client-ca.pem`.

## Scope

mTLS applies globally to all connections on the TLS port. There is no per-route
or per-virtual-host granularity at the TLS layer; use application middleware
(e.g., `JwtLayer` or a custom `Middleware`) for finer-grained access control
within a single TLS session.

:::note[Plain HTTP connections]
mTLS only applies to TLS connections. Plain HTTP/1.1 connections (when no cert
is configured, or connections on a redirect port) are not affected.
:::

## Configuration reference

| Variable | Config key | Description |
|---|---|---|
| `RWS_CONFIG_TLS_CLIENT_CA_FILE` | `tls_client_ca_file` | Path to the CA PEM file whose signatures are trusted for client certificates |
