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
| S2.forwarded | incomplete | Behind a TLS-terminating gateway rws sends `X-Forwarded-Proto: https` (the gateway's, believed) but its own `Forwarded: for=<gateway>;proto=http`, so the two disagree. | open |
