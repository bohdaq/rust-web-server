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

On first start `rws` requests a certificate, stores it in `.rws/acme/`
(`cert.pem`, `key.pem`, `account.key` — keys `0600`, the directory `0700`,
and never served: the working directory is the web root), and starts serving HTTPS, HTTP/2, and
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
- **One way for the CA to check you control the domains**: port 80 reachable
  from the internet (HTTP-01, the default), or port 443 (TLS-ALPN-01), or
  write access to the domain's DNS (DNS-01, also for wildcards) — see
  [Challenge types](#challenge-types).
- **Binding ports 80/443 on Linux** needs root or the capability:
  `sudo setcap cap_net_bind_service=+ep "$(which rws)"`.

IPv6 is handled: when a domain has an AAAA record, Let's Encrypt validates
over IPv6, so the challenge and redirect listeners accept both IPv4 and IPv6.

## How it works

1. **Startup.** If no certificate exists, or it is due for renewal (see 4),
   `rws` runs the ACME flow before opening the HTTPS
   port: account (key saved to `.rws/acme/account.key`), order, HTTP-01 challenge,
   ECDSA P-256 key + CSR, finalize, download.
2. **Challenges.** At startup a temporary listener on the challenge port
   (default 80) answers them. Once the HTTP → HTTPS redirect listener is
   running on that port, it answers challenges itself — so renewals never
   compete with it for port 80.
3. **First-time failures are retried**, e.g. while DNS for a new domain is
   still propagating: after 30 s, doubling up to every 15 minutes, until a
   certificate is issued.
4. **Renewal** (since 18.0.0). A certificate is renewed at two-thirds of its
   lifetime — day 60 of 90, day 30 of 45, day 4 of 6 — or earlier when the
   CA's ACME Renewal Information ([RFC 9773](https://www.rfc-editor.org/rfc/rfc9773.html))
   suggests a window that opens sooner, e.g. after a revocation. `rws` asks
   for it at every check (every 12 hours at most, every 6 when the CA has
   renewal information, sooner when renewal is due earlier or the CA's
   `Retry-After` says so) and picks a point in the window that's fixed per
   certificate. The renewal order names the certificate it replaces, so the
   CA can exempt it from rate limits. A failed renewal is retried within an
   hour. After renewing it reloads the TLS certificate in place (on Unix, via
   the same `SIGHUP` reload used for config). `renew_before_days`, if set,
   replaces the two-thirds point (a CA window that opens earlier still wins).
5. **Profiles.** `profile` orders a certificate profile the CA offers —
   Let's Encrypt has `classic`, `tlsserver` and `shortlived` (6-day
   certificates). A name the CA doesn't list stops provisioning with the list
   it offers. Unset, the CA picks its default.

The ACME client (RFC 8555) is implemented in-crate. It has been tested end to
end against [Pebble](https://github.com/letsencrypt/pebble), Let's Encrypt's
test CA, including a `shortlived` profile and renewal on Pebble's renewal
information through the redirect listener.

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

Delete `.rws/acme/` before switching back to production.

Before 17.113.1 the defaults were `cert.pem`, `key.pem` and `acme_account.key`
in the working directory — the web root, so the keys could be downloaded.
On upgrade, files found there are moved into `.rws/acme/` (when the default
paths are in use), and the static server no longer serves `*.pem`, `*.key`
or `rws.config.toml` anywhere. Rotate a key that was exposed.

## Private ACME CAs

To use step-ca, an internal Boulder, or Pebble, set the directory URL and
trust the CA's root certificate (in addition to the public roots):

```toml
[acme]
domains   = ["internal.example"]
directory = "https://ca.internal.example:9000/acme/acme/directory"
ca_file   = "/etc/rws/internal-root.pem"
```

## Challenge types

`challenge` picks how the CA checks that you control the domains (since
18.4.0; HTTP-01 before):

| `challenge` | The CA checks | Needs | Wildcards |
|---|---|---|---|
| `http-01` (default) | `http://<domain>/.well-known/acme-challenge/…` | port 80 reachable | no |
| `tls-alpn-01` | a TLS handshake on port 443 with ALPN `acme-tls/1` (RFC 8737) | port 443 reachable — nothing else | no |
| `dns-01` | a TXT record `_acme-challenge.<domain>` | a way to change the domain's DNS | yes |

### TLS-ALPN-01

For a server that only listens on 443 — no port 80 at all:

```toml
port = 443

[acme]
domains   = ["example.com"]
challenge = "tls-alpn-01"
```

rws answers on its HTTPS port: at startup, before it has a certificate, a
temporary acceptor on the same socket serves the challenge; afterwards the
TLS listener does, beside the normal traffic. A challenge handshake gets a
special self-signed certificate and is then closed; other clients never
see it. If a port mapping forwards 443 elsewhere, set `port` to where it
lands.

### DNS-01

The CA looks up a TXT record, so nothing needs to reach the server — and it
is the only way to get a wildcard certificate (`*.example.com`). rws adds
the record, waits until the zone's server answers with it, lets the CA check,
and removes it.

```toml
[acme]
domains   = ["example.com", "*.example.com"]
challenge = "dns-01"
dns_provider        = "rfc2136"            # DNS UPDATE (RFC 2136) to the primary server
dns_server          = "ns1.example.com"    # host[:port], port 53
dns_tsig_key        = "rws-acme"           # TSIG key name the server knows
dns_tsig_secret     = "base64secret=="     # better in RWS_CONFIG_ACME_DNS_TSIG_SECRET
# dns_tsig_algorithm  = "hmac-sha256"      # or hmac-sha512
# dns_zone            = "example.com"      # found by SOA lookup when not set
# dns_propagation_secs = 120               # how long to wait for the record
```

| `dns_provider` | Adds and removes the record by |
|---|---|
| `rfc2136` | a DNS UPDATE to `dns_server`, signed with the TSIG key — BIND, Knot, PowerDNS, Windows DNS, and many DNS hosts' "dynamic DNS" endpoints |
| `exec` | running `dns_exec` as `<program> present <fqdn> <value>` and `<program> cleanup <fqdn> <value>`: any DNS API, in a script (exit status 0 = done) |

A BIND key and the zone permission it needs:

```text
key "rws-acme" { algorithm hmac-sha256; secret "base64secret=="; };
zone "example.com" { type primary; file "example.com.zone";
    update-policy { grant rws-acme name _acme-challenge.example.com. TXT; }; };
```

(`tsig-keygen rws-acme` prints a new key.) The secret is never printed in
rws's startup log.

## On-demand TLS

For a SaaS whose customers point their own domains at it: rws orders a
certificate at the first TLS handshake for a name nobody configured, once
your *ask* endpoint allows that name (since 18.4.0).

```toml
http_redirect_port = 80                  # answers HTTP-01

[acme]
domains       = ["saas.example.com"]     # rws's own certificate, the default
on_demand     = true                     # RWS_CONFIG_ACME_ON_DEMAND
on_demand_ask = "http://127.0.0.1:9000/allowed"   # RWS_CONFIG_ACME_ON_DEMAND_ASK
```

- Before each order rws asks `GET <on_demand_ask>?domain=<name>`; a 2xx
  answer allows it, anything else (or no answer in five seconds) refuses
  it. Answer from your list of customer domains. `on_demand` without
  `on_demand_ask` is refused: anyone could make rws order certificates for
  every name they point at it.
- The first handshake for a new name waits for its certificate (up to 90
  seconds; with Let's Encrypt usually a few). A refused name gets the
  default certificate and isn't asked about again for a minute; a failed
  order isn't retried for ten. At most four orders run at once, and one per
  name however many clients arrive together.
- Certificates are kept in `.rws/acme/on-demand/<name>/` (next to the
  `cert_path`), loaded at startup and renewed like the main one; with
  [`redis`](#several-replicas) replicas share them too.
- HTTP-01 or TLS-ALPN-01 (DNS-01 would need every customer's zone). Over
  HTTP/3 a name is served once a TCP handshake has ordered it.

CI checks it against Pebble with an ask endpoint (`ci/e2e/acme_on_demand.sh`):
an allowed name is served its certificate on the first request, a refused
one the default, five simultaneous first requests make one order, and a
restart orders nothing.

## Several replicas

Replicas of one site behind a load balancer each run ACME for the same
domains. On their own they would each order a certificate — hitting the
CA's rate limits — and each answer only the challenges they presented,
while the CA's request may reach another replica. Give them one Redis
(since 18.4.0):

```toml
[acme]
domains = ["example.com"]
redis   = "10.0.0.20:6379"       # RWS_CONFIG_ACME_REDIS
# redis_password = "..."         # RWS_CONFIG_ACME_REDIS_PASSWORD; never printed
# redis_prefix   = "rws:acme"    # several sites in one Redis
```

- One replica orders each certificate (a lock in Redis, held up to five
  minutes); the others wait for it and install it.
- A renewal by any replica reaches the rest at their next renewal check,
  which they hot-swap like their own.
- HTTP-01 and TLS-ALPN-01 challenges are answered by every replica, from
  Redis, wherever the CA's request lands.
- Redis down: each replica carries on alone with its own files, and says so
  in its log.

The certificate's private key is stored in Redis: keep Redis on a private
network (and set a password). CI runs two replicas against one Redis and
Pebble (`ci/e2e/acme_shared.sh`): one order per certificate, both serving
the same one before and after a renewal.

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
| `challenge` | `RWS_CONFIG_ACME_CHALLENGE` | `http-01` | `http-01`, `tls-alpn-01` or `dns-01` — see [Challenge types](#challenge-types) |
| `challenge_port` | `RWS_CONFIG_ACME_CHALLENGE_PORT` | `80` | Port the CA connects to for HTTP-01 |
| `dns_provider` | `RWS_CONFIG_ACME_DNS_PROVIDER` | — | DNS-01: `rfc2136` or `exec` |
| `dns_server` | `RWS_CONFIG_ACME_DNS_SERVER` | — | DNS-01: `rfc2136`'s primary server, `host[:port]`; with `exec`, optional: where to look for the record (else the system resolver) |
| `dns_zone` | `RWS_CONFIG_ACME_DNS_ZONE` | SOA lookup | DNS-01: the zone of the records |
| `dns_tsig_key` | `RWS_CONFIG_ACME_DNS_TSIG_KEY` | — | DNS-01 `rfc2136`: TSIG key name |
| `dns_tsig_secret` | `RWS_CONFIG_ACME_DNS_TSIG_SECRET` | — | DNS-01 `rfc2136`: TSIG secret, base64 (no command-line flag; never printed) |
| `dns_tsig_algorithm` | `RWS_CONFIG_ACME_DNS_TSIG_ALGORITHM` | `hmac-sha256` | DNS-01 `rfc2136`: `hmac-sha256` or `hmac-sha512` |
| `dns_exec` | `RWS_CONFIG_ACME_DNS_EXEC` | — | DNS-01 `exec`: the program |
| `dns_propagation_secs` | `RWS_CONFIG_ACME_DNS_PROPAGATION_SECS` | `120` | DNS-01: how long to wait for the record to be visible |
| `redis` | `RWS_CONFIG_ACME_REDIS` | — | Redis `host:port` shared by replicas — see [Several replicas](#several-replicas) |
| `redis_password` | `RWS_CONFIG_ACME_REDIS_PASSWORD` | — | Its password (no command-line flag; never printed) |
| `redis_prefix` | `RWS_CONFIG_ACME_REDIS_PREFIX` | `rws:acme` | Key prefix in that Redis |
| `on_demand` | `RWS_CONFIG_ACME_ON_DEMAND` | `false` | Order certificates at the first handshake — see [On-demand TLS](#on-demand-tls) |
| `on_demand_ask` | `RWS_CONFIG_ACME_ON_DEMAND_ASK` | — | URL asked `?domain=<name>` before each such order; 2xx allows it |
| `cert_path` | `RWS_CONFIG_ACME_CERT_PATH` | `tls_cert_file`, else `.rws/acme/cert.pem` | Where the certificate chain is stored |
| `key_path` | `RWS_CONFIG_ACME_KEY_PATH` | `tls_key_file`, else `.rws/acme/key.pem` | Where the certificate key is stored (`0600`) |
| `renew_before_days` | `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | — | Renew this many days before expiry instead of at two-thirds of the lifetime (was `30` before 18.0.0) |
| `profile` | `RWS_CONFIG_ACME_PROFILE` | — | Certificate profile to order (`classic`, `tlsserver`, `shortlived`, … as the CA offers) |
| `renew_check_interval_secs` | `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` | `43200` | Longest time between renewal checks |
| `account_key_path` | `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `.rws/acme/account.key` | Where the ACME account key is stored (`0600`) |

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
