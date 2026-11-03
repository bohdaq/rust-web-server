[Read Me](../README.md) > [Spec](.) > Stage E2E findings

# Stage E2E findings

What the stage end-to-end suite (`ci/stage/`, design: the "rws on kube-dc
stage — E2E test design" doc) found, one row per finding, keyed by case id so
a rerun shows what is fixed. Kinds: **bug** (wrong behaviour), **gap**
(documented or expected but missing), **incomplete** (partly works),
**blocked** (a kube-dc fault, not rws).

| Case | Kind | Finding | Status |
|---|---|---|---|
| H0 chart config | gap | The Helm chart had no way to give rws a `rws.config.toml`: no ConfigMap mount, no extra volumes, so a chart user couldn't configure routes, upstreams or middleware at all (env vars only). | fixed: `configFile`, `configMountPath`, `extraVolumes`, `extraVolumeMounts` (unreleased) |
| S3.ip client address | bug | Behind the kube-dc gateway every connection comes from its address (172.30.0.3), and rws keyed the rate limit, IP filter, blocklist, WAF IP rules, `ip_hash` and the logs on it: all clients shared one rate-limit bucket, `allow_ips` couldn't name a client. | fixed: the client from a trusted proxy's `X-Forwarded-For`, `client_ip = "peer"` to opt out (unreleased) |
| S2.timeout | bug | A backend slower than `read_timeout_ms` got `502 Bad Gateway`; the design (and nginx) say `504`, and `Proxy-Status` already said `http_response_timeout`. | fixed (unreleased) |
| S2.forwarded | incomplete | Behind a TLS-terminating gateway rws sends `X-Forwarded-Proto: https` (the gateway's, believed) but its own `Forwarded: for=<gateway>;proto=http`, so the two disagree. | fixed (unreleased) |
| S3.3 basic challenge | bug | Basic auth answered wrong credentials with a bare `401`: no `WWW-Authenticate`, so a browser shows the error instead of asking again. | fixed (unreleased) |
| S1.6 ETag per replica | incomplete | Static ETags come from the file's mtime, which differs per pod for the same ConfigMap (or image layer unpacked at different times): `If-None-Match` gets a `304` only from the pod that made the ETag, 1 in 3 behind the gateway. | fixed (unreleased) |
| S1.18 CORS on routes | bug | `[cors]` ("applied to every response") is only applied to static files: `[[route]]` responses (respond, proxy) get no `Access-Control-*` headers and a preflight `OPTIONS` goes to the route. | fixed (unreleased) |
| S8.4 public metrics | gap | `/metrics` is served on the main listener to anyone: through the public gateway it shows upstream names, backend addresses and traffic counts. There's no setting to keep it to scrapers. | fixed (unreleased) |
| S8.9 drain | incomplete | One run of a rolling restart under traffic got one `503` from the gateway (699 × 200): on `SIGTERM` rws stops accepting at once, before the gateway has taken the pod's endpoint out. A second run was clean. | fixed (unreleased) |
