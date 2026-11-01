---
title: HTTPS / TLS
description: Serve traffic over TLS with HTTP/2 and HTTP/1.1 on the same port using rustls and aws-lc-rs.
---

## Feature requirements

TLS is available when you build with the `http2` or `http3` (default) feature.
The `http1`-only build has no TLS support.

```bash
# Default build — HTTP/3 + HTTP/2 + TLS
cargo build

# HTTP/2 + TLS only, no QUIC
cargo build --no-default-features --features http2
```

## Local HTTPS: `tls = "internal"`

For development, rws can be its own certificate authority (since 18.21.0):

```sh
rws --tls=internal          # or tls = "internal" in rws.config.toml
rws trust                   # once: browsers and curl trust https://localhost
```

On first use rws creates a CA (EC P-256, valid ten years) in your data
directory — `~/.local/share/rws/pki` on Linux (`$XDG_DATA_HOME` if set),
`~/Library/Application Support/rws/pki` on macOS, `%APPDATA%\rws\pki` on
Windows; `tls_internal_dir` (`RWS_CONFIG_TLS_INTERNAL_DIR`) moves it. It then
serves a certificate it issued for `localhost`, `127.0.0.1`, `::1`, the
listen address, every `[[virtual_host]]` domain and the names in
`tls_internal_names`:

```toml
tls = "internal"
tls_internal_names = "app.localhost, api.localhost, myapp.test"
```

The certificate lives 7 days and is re-issued at half-life without a
restart. Browsers refuse wildcards directly under a top-level name, so list
`app.localhost` rather than `*.localhost` (`*.myapp.test` is fine).

`rws trust` adds the CA to the system trust store — `update-ca-certificates`
on Debian/Ubuntu, `update-ca-trust` on Fedora/RHEL, `trust` on Arch (with
`sudo`, which asks for your password), the System keychain on macOS, the
current user's Root store on Windows — and to the NSS database Chrome and
Firefox use on Linux (`~/.pki/nssdb`, needs `certutil` from
`libnss3-tools`). `rws untrust` takes it out; `rws trust --print` writes the
CA certificate to stdout, e.g. for a phone or another machine:

```sh
rws trust --print > rws-ca.pem
curl --cacert rws-ca.pem https://localhost:7878/
```

Only for development and private networks: the CA key sits on your disk.
If `tls_cert_file`/`tls_key_file` or `acme_domains` are set, they win and
`tls = "internal"` is ignored (with a note at startup).

## Generating a self-signed certificate

Or use `openssl` to create a certificate and key yourself:

```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
```

## Starting the server with TLS

Pass the certificate and key on the command line:

```bash
cargo run -- --tls-cert-file=cert.pem --tls-key-file=key.pem
```

Alternatively, set environment variables or add them to `rws.config.toml`:

```bash
export RWS_CONFIG_TLS_CERT_FILE=cert.pem
export RWS_CONFIG_TLS_KEY_FILE=key.pem
cargo run
```

```toml
# rws.config.toml
tls_cert_file = "cert.pem"
tls_key_file  = "key.pem"
```

## TLS implementation

`rust-web-server` uses [rustls](https://github.com/rustls/rustls) with the
`aws-lc-rs` cryptography backend. There is no dependency on OpenSSL.

## FIPS 140-3

For environments that require FIPS-validated cryptography, build with the
`fips` feature (since 18.23.0) or use the release's
`rws-<version>-x86_64-unknown-linux-gnu-fips.tar.gz`:

```sh
cargo install rust-web-server --features "fips acme auth sso"   # needs CMake, Go and a C compiler
rws --version   # rws 18.23.0 (FIPS)
```

TLS — the HTTPS and QUIC listeners, mTLS client verification, ACME, the
reverse proxy's upstream connections and the outbound HTTP client — then
runs on aws-lc's FIPS 140-3 validated module with only FIPS-approved
algorithms: AES-GCM suites (no ChaCha20-Poly1305), NIST curves and the
approved hybrid key exchange, RSA and ECDSA signatures. At startup rws
checks that every TLS configuration rustls builds reports itself
FIPS-compliant and refuses to start otherwise; `GET /admin/api/v1/status`
lists `fips` among the features.

Not covered by the module: HS256 JWTs and the `{SHA256}` htpasswd scheme
(RustCrypto `hmac`/`sha2`), ES256 JWT verification (`p256`), rws's own
SHA-256 for cache keys and signed cookies, and the in-crate RSA verifier
used for RS256 tokens and SAML. Where your policy needs those under the
module too, keep authentication in a FIPS-validated identity provider and
use the TLS side of rws. The FIPS build is glibc-only (aws-lc's FIPS module
doesn't build for musl), so there's no FIPS container image yet.

## Keys in an HSM or cloud KMS (PKCS#11)

Since 18.23.0 the private key can stay on a hardware security module, a
smart card or a cloud KMS: `tls_key_file` (or a virtual host's `key_file`)
takes an RFC 7512 `pkcs11:` URI instead of a path, and rws asks the token to
sign each handshake.

```toml
tls_cert_file = "/etc/rws/cert.pem"          # the certificate stays a file
tls_key_file  = "pkcs11:token=rws;object=web-key?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-source=env:HSM_PIN"
```

| URI part | Meaning |
|---|---|
| `token=` | the token's label |
| `object=` / `id=` | the key's label / its `CKA_ID` (percent-encoded bytes) |
| `module-path=` | the vendor's PKCS#11 library |
| `pin-source=env:VAR` | the PIN from an environment variable (or `file:/run/secrets/pin`); `pin-value=` works but puts the PIN in the config |

Keys: ECDSA P-256 and P-384, RSA (RSA-PSS for TLS 1.3, PKCS#1 v1.5 also for
TLS 1.2 clients). The module loads at startup; each key keeps a pool of up
to 8 logged-in sessions (handshakes sign in parallel), reopened if the token
drops them; a `pin-value=` is hidden in the startup output and the admin
API's config; and `rws --check` opens the
key, so a wrong PIN or label shows up before a restart. Tested with
SoftHSM2; the same works with AWS CloudHSM (`libcloudhsm_pkcs11.so`),
Google Cloud KMS (`libkmsp11.so`, with its config file in the environment),
Azure Managed HSM and YubiHSM's PKCS#11 libraries.

Needs the `pkcs11` feature: in the macOS, Windows and Linux glibc
(`…-linux-gnu-fips`) release binaries, not in the static musl ones (they
can't load a library at runtime) — or `cargo install rust-web-server
--features "pkcs11 acme auth"`. With `fips` too, TLS runs on aws-lc's FIPS
module and the key never leaves the HSM.

## ALPN negotiation

When a TLS certificate is configured, the server advertises both `h2` and
`http/1.1` via ALPN in the TLS handshake. A single port handles HTTP/2 and
HTTP/1.1 simultaneously — no extra configuration is required.

```
alpn_protocols = ["h2", "http/1.1"]
```

The HTTP/2 server (`h2_server`) turns HTTP/2 streams into the same `Request` /
`Application::execute` / `Response` pipeline that HTTP/1.1 uses.

## HTTP → HTTPS redirect

Set `RWS_CONFIG_HTTP_REDIRECT_PORT` to have the server also listen on a plain
HTTP port and issue `301 Moved Permanently` redirects to HTTPS:

```bash
export RWS_CONFIG_HTTP_REDIRECT_PORT=80
```

```toml
# rws.config.toml
http_redirect_port = "80"
```

`Server::run_redirect()` binds the redirect listener and sends every incoming
request to the HTTPS port with a `301` response.

## Alt-Svc advertisement

HTTP/1.1 TLS responses include an `Alt-Svc` header so clients learn that a
faster protocol is available:

- HTTP/3 build: `Alt-Svc: h3=":7878"`
- HTTP/2-only build: `Alt-Svc: h2=":7878"`

Browsers that support HTTP/3 will upgrade automatically on subsequent requests.

## Configuration reference

| Variable | Config key | Description |
|---|---|---|
| `RWS_CONFIG_TLS_CERT_FILE` | `tls_cert_file` | Path to the PEM certificate chain |
| `RWS_CONFIG_TLS_KEY_FILE` | `tls_key_file` | Path to the PEM private key |
| `RWS_CONFIG_HTTP_REDIRECT_PORT` | `http_redirect_port` | Plain HTTP port that issues 301 → HTTPS |

:::note[Hot reload]
Send `SIGHUP` or `POST /admin/config/reload` to reload the TLS certificate
from disk without restarting the server. This also rebuilds the TLS acceptor
for all configured virtual hosts.
:::


## Upstream TLS (proxy mode)

When using the config-driven proxy (`rws.config.toml`), prefix backend addresses with `https://` to connect to upstream services over TLS:

```toml
[[upstream]]
name     = "secure-api"
backends = ["https://api.internal:443", "https://api2.internal:443"]
```

Certificate verification uses the WebPKI root store (same trust anchors as browsers). To trust a private CA instead, set `tls_ca_file` on the `[[upstream]]` (since 18.0.0). Requires the `http-client` or `http2` feature — included in all builds except `--no-default-features --features http1`.

See [Config-Driven Proxy → TLS upstreams](/proxy/config-driven/#tls-https-upstreams) for the full reference.
