---
title: Automatic HTTPS (ACME)
description: Get and auto-renew Let's Encrypt certificates with one config section — no certbot, no cron, no restarts.
---

`rws` obtains certificates from Let's Encrypt (or any ACME CA) by itself and
keeps them renewed. Point a domain's DNS at the server, then:

```toml
# rws.config.toml
port               = 443
http_redirect_port = 80     # HTTP → HTTPS redirects; also answers ACME challenges

[acme]
domains = ["example.com", "www.example.com"]
email   = "you@example.com"
```

```bash
rws
```

On first start `rws` requests a certificate, stores it as `cert.pem` /
`key.pem` in the working directory, and starts serving HTTPS, HTTP/2, and
HTTP/3 with it. Later starts reuse the stored certificate. Renewal happens in
the background and is hot-swapped without dropping connections.

:::note[Which binaries include ACME]
The prebuilt binaries (Homebrew, `cargo binstall`, Docker, GitHub Releases)
include it. When building from source, enable the `acme` feature:
`cargo install rust-web-server --features acme,auth`. A binary built without
it prints a warning if `[acme]` is configured instead of silently serving
plain HTTP.
:::

## Requirements

- **DNS** for every domain in `domains` points at this server.
- **Port 80 is reachable from the internet.** Let's Encrypt proves you control
  the domain with an HTTP-01 challenge to `http://<domain>/.well-known/acme-challenge/…`.
- **Binding ports 80/443 on Linux** needs root or the capability:
  `sudo setcap cap_net_bind_service=+ep "$(which rws)"`.

IPv6 is handled: when a domain has an AAAA record, Let's Encrypt validates
over IPv6, so the challenge and redirect listeners accept both IPv4 and IPv6.

## How it works

1. **Startup.** If no certificate exists, or it expires within
   `renew_before_days`, `rws` runs the ACME flow before opening the HTTPS
   port: account (key saved to `acme_account.key`), order, HTTP-01 challenge,
   ECDSA P-256 key + CSR, finalize, download.
2. **Challenges.** At startup a temporary listener on the challenge port
   (default 80) answers them. Once the HTTP → HTTPS redirect listener is
   running on that port, it answers challenges itself — so renewals never
   compete with it for port 80.
3. **First-time failures are retried**, e.g. while DNS for a new domain is
   still propagating: after 30 s, doubling up to every 15 minutes, until a
   certificate is issued.
4. **Renewal.** Every 12 hours `rws` checks the expiry and renews when fewer
   than `renew_before_days` (30) remain. A failed renewal is retried within an
   hour. After renewing it reloads the TLS certificate in place (on Unix, via
   the same `SIGHUP` reload used for config).

The ACME client (RFC 8555) is implemented in-crate. It has been tested end to
end against [Pebble](https://github.com/letsencrypt/pebble), Let's Encrypt's
test CA, including renewal through the redirect listener.

## Testing with the staging CA

Let's Encrypt production has strict
[rate limits](https://letsencrypt.org/docs/rate-limits/). Try your setup
against staging first — its certificates aren't browser-trusted, but the
limits are generous:

```toml
[acme]
domains = ["example.com"]
staging = true
```

Delete `cert.pem`, `key.pem`, and `acme_account.key` before switching back to
production.

## Private ACME CAs

To use step-ca, an internal Boulder, or Pebble, set the directory URL and
trust the CA's root certificate (in addition to the public roots):

```toml
[acme]
domains   = ["internal.example"]
directory = "https://ca.internal.example:9000/acme/acme/directory"
ca_file   = "/etc/rws/internal-root.pem"
```

## Configuration reference

Every setting can go in the `[acme]` section of `rws.config.toml`, on the
command line (`--acme-domains=...`; not the last three), or in the environment.

| `[acme]` key | Environment variable | Default | Description |
|---|---|---|---|
| `domains` | `RWS_CONFIG_ACME_DOMAINS` | — | Domains for the certificate. Setting this enables ACME. |
| `email` | `RWS_CONFIG_ACME_EMAIL` | — | Contact email for expiry notices from the CA |
| `staging` | `RWS_CONFIG_ACME_STAGING` | `false` | Use the Let's Encrypt staging CA |
| `directory` | `RWS_CONFIG_ACME_DIRECTORY` | Let's Encrypt | ACME directory URL of another CA |
| `ca_file` | `RWS_CONFIG_ACME_CA_FILE` | — | Extra root CA(s) to trust for the directory (private CAs) |
| `challenge_port` | `RWS_CONFIG_ACME_CHALLENGE_PORT` | `80` | Port the CA connects to for HTTP-01 |
| `cert_path` | `RWS_CONFIG_ACME_CERT_PATH` | `tls_cert_file`, else `cert.pem` | Where the certificate chain is stored |
| `key_path` | `RWS_CONFIG_ACME_KEY_PATH` | `tls_key_file`, else `key.pem` | Where the certificate key is stored |
| `renew_before_days` | `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | `30` | Renew when fewer than this many days remain |
| `renew_check_interval_secs` | `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` | `43200` | Seconds between expiry checks |
| `account_key_path` | `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `acme_account.key` | Where the ACME account key is stored |

Top-level `http_redirect_port` (`RWS_CONFIG_HTTP_REDIRECT_PORT`) starts the
HTTP → HTTPS redirect listener; set it to the challenge port (80) in
production.

## Using ACME from Rust

The `rws` binary does all of the above. In your own binary built on the
library, the equivalent is:

```rust
use rust_web_server::acme::{AcmeConfig, AcmeManager};
use rust_web_server::prelude::*;

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();

    if let Some(cfg) = AcmeConfig::from_env() {
        // Serve the issued certificate unless TLS paths were set explicitly.
        cfg.install_as_tls_certificate();
        let mgr = AcmeManager::new(cfg);
        mgr.provision_until_ready().await;   // retries until issued
        tokio::spawn(mgr.run_renewal_loop());
    }

    let app = App::new();
    tokio::join!(
        Server::run_tls(listener, pool, app.clone()),
        Server::run_quic(app),
        Server::run_redirect(),               // answers renewal challenges
    );
}
```

`AcmeManager::provision_if_needed()` (single attempt, returns the error) is
also available if you want to handle failures yourself.
