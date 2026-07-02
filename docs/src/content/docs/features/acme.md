---
title: Automatic TLS (ACME)
description: Provision and auto-renew Let's Encrypt certificates via the ACME protocol without any external tooling.
---

## Feature requirements

Automatic certificate management requires the `acme` feature, which implies
`http2`:

```bash
cargo build --features acme
```

The `http3` default build does **not** include ACME; add it explicitly:

```bash
cargo build --features http3,acme
```

## How it works

`AcmeManager` implements RFC 8555 (ACME) from scratch with no third-party ACME
client libraries. On startup it checks whether a valid certificate already
exists. If not, or if the certificate expires within `renew_before_days`, it:

1. Loads or generates an ACME account key (`acme_account.key` by default).
2. Creates or looks up an account on the CA directory.
3. Submits a new order for all configured domains.
4. For each domain, starts a temporary HTTP-01 challenge server on port 80,
   responds to `GET /.well-known/acme-challenge/<token>`, then polls until
   Let's Encrypt validates.
5. Generates an ECDSA P-256 key pair and a CSR, finalises the order, and
   downloads the signed certificate chain.
6. Writes the chain to `cert.pem` (or `RWS_CONFIG_ACME_CERT_PATH`) and the
   private key to `key.pem` (or `RWS_CONFIG_ACME_KEY_PATH`).

## Quick start

```bash
export RWS_CONFIG_ACME_DOMAINS=example.com,www.example.com
export RWS_CONFIG_ACME_EMAIL=admin@example.com
export RWS_CONFIG_TLS_CERT_FILE=cert.pem
export RWS_CONFIG_TLS_KEY_FILE=key.pem
```

```rust
use rust_web_server::acme::{AcmeConfig, AcmeManager};
use rust_web_server::{App, Server};

#[tokio::main]
async fn main() {
    // Provision certificate before starting the TLS server.
    if let Some(cfg) = AcmeConfig::from_env() {
        let mgr = AcmeManager::new(cfg);
        mgr.provision_if_needed().await.unwrap();

        // Renew in the background every 12 hours.
        tokio::spawn(mgr.run_renewal_loop());
    }

    Server::setup().run_tls(App::new()).await;
}
```

## Background renewal

`AcmeManager::run_renewal_loop()` sleeps for 12 hours, then calls
`provision_if_needed()`. If renewal succeeds it sends `SIGHUP` to the running
process, which triggers `Server::run_tls` to hot-reload the TLS acceptor with
the new certificate — no restart required.

```rust
tokio::spawn(mgr.run_renewal_loop());
```

## Staging vs production

Set `RWS_CONFIG_ACME_STAGING=true` to use the Let's Encrypt staging environment
during development. Staging certificates are not trusted by browsers but the CA
has much higher rate limits.

```bash
export RWS_CONFIG_ACME_STAGING=true
```

| CA | URL |
|---|---|
| Production | `https://acme-v02.api.letsencrypt.org/directory` |
| Staging | `https://acme-staging-v02.api.letsencrypt.org/directory` |

Set `RWS_CONFIG_ACME_DIRECTORY` to use a different ACME-compatible CA.

## HTTP-01 challenge server

The ACME manager binds a temporary TCP listener on port 80 (configurable via
`RWS_CONFIG_ACME_CHALLENGE_PORT`) for the duration of each domain's HTTP-01
challenge. The listener is shut down as soon as Let's Encrypt confirms the
challenge is valid. Port 80 must be reachable from the public internet during
provisioning.

:::note[Port 80 binding]
On Linux, binding port 80 without root privileges requires either
`CAP_NET_BIND_SERVICE` or running the process as root. You can set the
capability with: `sudo setcap cap_net_bind_service=+ep ./rws`
:::

## Configuration reference

| Variable | Default | Description |
|---|---|---|
| `RWS_CONFIG_ACME_DOMAINS` | — | Comma-separated domain list (required to activate ACME) |
| `RWS_CONFIG_ACME_EMAIL` | — | Contact email sent to the CA |
| `RWS_CONFIG_ACME_STAGING` | `false` | Use Let's Encrypt staging when `true` |
| `RWS_CONFIG_ACME_DIRECTORY` | LE production URL | Custom ACME directory URL |
| `RWS_CONFIG_ACME_CERT_PATH` | `RWS_CONFIG_TLS_CERT_FILE` | Where to write the certificate chain |
| `RWS_CONFIG_ACME_KEY_PATH` | `RWS_CONFIG_TLS_KEY_FILE` | Where to write the certificate private key |
| `RWS_CONFIG_ACME_CHALLENGE_PORT` | `80` | Port for the HTTP-01 challenge server |
| `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | `30` | Renew when fewer than this many days remain |
| `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `acme_account.key` | Persist the ACME account key between restarts |
