[Read Me](../README.md) > [Spec](.) > Gaps V4

# Gaps V4 — Kubernetes Adoption & Cloud Providers

`K8S_ROADMAP.md` tracked the minimum for a pod to run and be probed correctly — all six items there are closed (0.0.0.0 bind, `/healthz`/`/readyz`, SIGTERM drain, Dockerfile, JSON logs, `/metrics`). `GAPS.md`'s "Kubernetes / cloud-native gaps" section then added ingress, service discovery, canary routing, and circuit breaking as middleware — all four shipped, each with its own "Remaining" list. This document (audited against v17.94.0) rolls those remaining items up alongside the gaps that are specific to running on a *managed cloud provider* (AWS/GCP/Azure) rather than bare Kubernetes: object storage parity, secrets management, and shippable deployment artifacts.

---

## Part 1 — Kubernetes middleware, remaining items

### ✅ 1.1 Ingress controller (`src/ingress/mod.rs`) — Done

Working today: polls `/apis/networking.k8s.io/v1/ingresses`, builds a live route table, routes to `{service}.{namespace}.svc.cluster.local:{port}`.

**What was missing (all four now closed):**
- TLS to `kubernetes.default.svc` — the watcher talks to the API server over plain HTTP today; needs a rustls client config trusting the in-cluster CA (`/var/run/secrets/kubernetes.io/serviceaccount/ca.crt`).
- Watch API (`?watch=true`) instead of fixed-interval polling — polling means route changes lag by up to the poll interval and the watcher does needless work on quiet clusters.
- `pathType: Exact` support — only prefix matching is implemented.
- IngressClass filtering — the watcher currently picks up every Ingress object cluster-wide regardless of `spec.ingressClassName`, which breaks multi-controller clusters (e.g. rws running alongside nginx-ingress).

**How each was closed, and one bug found along the way:**

1. **TLS**: `KubernetesIngressWatcher::from_service_account()` — previously a stub always returning `Err` — now really connects to `https://kubernetes.default.svc`, gated `#[cfg(any(feature = "http-client", feature = "http2"))]` (`src/ingress/tls.rs`, new). Trusts *only* the CA at `.../ca.crt` (a hand-rolled PEM parser, not `rustls-pemfile`, since that's only available under `http2` and this needs to also work under `http-client` alone — consistent with this crate's established "hand-roll rather than add a dependency" pattern). Finds the API server via `KUBERNETES_SERVICE_HOST`/`KUBERNETES_SERVICE_PORT`, the same mechanism every real Kubernetes client library uses. Validated with a genuine TLS handshake in tests — not just parsed-and-assumed-correct — against a local `rustls`-backed listener presenting a real, `openssl`-generated, CA-signed certificate, including a negative test proving a server signed by an *unrelated* CA is correctly rejected.
2. **Watch API**: `src/ingress/watch.rs` (new) decodes the `Transfer-Encoding: chunked` watch stream into lines and treats *any* event line as a trigger to re-run the existing full LIST, rather than incrementally applying `ADDED`/`MODIFIED`/`DELETED` deltas to an in-memory cache — a deliberate scope decision (full delta tracking needs `resourceVersion` bookkeeping and correct `410 Gone` handling, meaningfully more surface area to get subtly wrong). Still delivers what this gap asked for: a quiet cluster leaves the watch thread blocked on a read with zero polling cost, and a real change is picked up as soon as the API server sends it, not after up to `poll_interval_secs`. The original interval resync keeps running unchanged alongside it as a safety net.
3. **`pathType: Exact`**: `IngressRule` gained a `path_type: PathType` field (`Prefix`/`Exact`/`ImplementationSpecific`), parsed from each path entry. Fixing this surfaced a **second, pre-existing bug in `Prefix` matching itself**: it was raw `str::starts_with`, so a rule for `/foo` incorrectly matched `/foobar` — Kubernetes' own `Prefix` semantics are element-wise (path-segment) matching, not a raw byte prefix. Fixed alongside `Exact`, since shipping a spec-correct `Exact` next to a known-non-spec-compliant `Prefix` would have been an odd half-measure.
4. **IngressClass filtering**: `.ingress_class(name)` builder + `ingress_class: Option<&str>` parameter on `parse_ingress_list` (a signature change to a public function, judged acceptable for a pre-1.0, fast-iterating crate). Unset (default) accepts every class, preserving today's behavior; an Ingress with no `ingressClassName` at all never matches a configured filter.
5. **Bonus bug fix, found while touching the parser for #4**: `parse_ingress_list`'s namespace extraction searched for `"namespace"` in the text *after* each `"spec"` occurrence — but `metadata` (and the `namespace` field inside it) always comes *before* `spec` in a real Kubernetes object's JSON encoding, so the search never actually found it and silently fell back to the `"default"` placeholder for every real API response, regardless of the Ingress's actual namespace (invisible unless the real namespace happened to also be `"default"`). No existing test caught this because none asserted on `.namespace` for a non-default-namespace fixture. Fixed by searching *backward* from each `"spec"` occurrence instead — matching what the function's own pre-existing (but never-implemented) comment already said the intent was.

Two new modules (`tls.rs`, `watch.rs`), 37 new/updated tests (real TLS handshakes — positive and negative — chunked-stream reassembly, `pathType` boundary semantics, class filtering, and a dedicated regression test for the namespace bug), full three-way build (default/`http1`/`http2`) green, no new Cargo dependency (PEM parsing and base64 decoding are hand-rolled, matching `rustls`/`webpki-roots` already being optional deps behind `http-client`/`http2`).

### ✅ 1.2 Service discovery (`src/service_discovery/mod.rs`) — Done

Working today: `Static`, `EnvPrefix`, `File`, `Dns` sources feeding a shared `Arc<RwLock<Vec<String>>>`.

**What was missing (all five now closed):** Consul HTTP API source, etcd watch source, Docker label discovery, DNS SRV record support (needed for headless Services that publish port info via SRV, not just A records), weighted DNS.

**How each was closed:**

1. **Consul**: `DiscoverySource::Consul { addr, service }` (`src/service_discovery/consul.rs`, new) queries a Consul agent's `/v1/health/service/:name?passing=true` — health filtering happens server-side, so only passing instances ever come back. `Service.Address` is preferred; falls back to `Node.Address` when a service registered without one (matches Consul's own documented resolution order). Uses `crate::http_client::Client`, which needs no feature flag — `service_discovery` itself has none, so it can only depend on always-available pieces.
2. **DNS SRV + weighted DNS**: `DiscoverySource::DnsSrv { record }` (`src/service_discovery/dns_srv.rs`, new) is a from-scratch RFC 1035/2782 query/response codec over a raw `UdpSocket` — no third-party DNS crate, including compression-pointer decoding for the target name. Only the lowest-priority tier of SRV answers is kept (RFC 2782: clients try that tier first); within it, each `target:port` is repeated `weight.clamp(1, 20)` times, which *is* this gap's "weighted DNS" — the mechanism by which a flat round-robin `Vec<String>` consumer ends up favoring higher-weight targets proportionally, since SRV carries a `weight` field plain A records don't.
3. **Docker label discovery**: `DiscoverySource::Docker { label, socket_path }` (`src/service_discovery/docker.rs`, new) queries the Docker Engine API over its Unix socket for running containers carrying `label`, using the label's **value** as the backend address directly (e.g. `rws.backend=10.0.0.5:8080`) rather than guessing an address from published ports or network topology — deliberately sidesteps that ambiguity, and keeps the parsing logic trivially unit-testable. Unix-only (`#[cfg(unix)]`); logs a warning and returns empty elsewhere.
4. **etcd watch**: `DiscoverySource::EtcdWatch { endpoints, prefix }` (`src/service_discovery/etcd.rs`, new) is the one source that isn't poll-driven — `BackendPool::start()` special-cases it to spawn a dedicated thread instead of the generic sleep-loop. After an initial one-shot `/v3/kv/range` listing (so `resolve()`/`refresh()` still work even without calling `start()`), the thread holds a long-lived connection to etcd's gRPC-gateway `/v3/watch` endpoint and applies `PUT`/`DELETE` events to the backend list incrementally as they arrive. It reuses `crate::ingress::watch::read_chunked_lines` (widened from `mod watch;` to `pub(crate) mod watch;` in `src/ingress/mod.rs` for this) — the same chunked-NDJSON-stream reader §1.1's Kubernetes watch already implemented, since both protocols shape their event streams identically. Unlike that watcher — which treats every event as a plain "something changed, re-list" trigger, because a Kubernetes `WatchEvent` doesn't carry enough to update one cached object in isolation — an etcd watch event *does* carry a complete key+value, so it's applied as a real incremental delta against a local map here. Plain HTTP only, no TLS yet — noted as a limitation, not silently assumed.
5. **Shared JSON parsing**: Consul/Docker/etcd responses are parsed by a new hand-rolled recursive-descent JSON value parser (`src/service_discovery/json_lite.rs`, crate-internal, not exposed) rather than the optional `serde`/`serde_json` feature — `service_discovery` has no feature gate of its own and must stay usable in every build, so it couldn't depend on an optional one. Mirrors this crate's existing per-module "own tiny encoders" pattern (`mcp::json_rpc`, `sso::saml::xml`) rather than introducing a crate-wide JSON type.

Five new modules (`json_lite.rs`, `consul.rs`, `dns_srv.rs`, `docker.rs`, `etcd.rs`), 108 new tests: a full JSON-parser test suite; Consul against a mock TCP HTTP server; DNS SRV packet encode/decode (including a crafted compression-pointer test and a pointer-loop rejection test) plus a live round-trip against a mock UDP resolver; Docker container-label parsing, chunked-response decoding, and a real `UnixListener`-backed end-to-end test; etcd's `prefix_range_end`/base64 round-trip, `apply_watch_line` delta application (PUT/DELETE, absent-`type`-defaults-to-PUT), and a full `run_once` end-to-end test against a two-request mock etcd server (initial list, then one watch event, verified via `pool.backends()`). Verified across default, `http1`-only, and default+`http1` feature combinations — 1511 tests green, zero regressions.

### ✅ 1.3 Traffic splitting / canary (`src/canary/mod.rs`) — Done

Working today (before this fix): `CanaryLayer` distributes requests across backends by static weight via a precomputed rotation vec.

**What was missing (all three now closed):** live weight updates without a restart (weights were baked in at construction), smooth weighted round-robin (the old scheme could burst same-backend requests within a rotation cycle for skewed weights), integration with `BackendPool` so canary targets can come from a dynamic discovery source instead of a fixed list.

**How each was closed:**

1. **Smooth weighted round-robin**: replaced the flat pre-expanded `Vec<(host, port, tls)>` rotation (a `weight=5` backend literally appeared 5 times consecutively in the vec, so an incrementing counter walking through it produced `AAAAA B C` — a real burst) with nginx's SWRR algorithm: each backend keeps a `current_weight` that accumulates its configured weight every selection and is decremented by the total weight when picked. Weights `5, 1, 1` now select roughly `A A B A C A A` (repeating) — verified by a test asserting the longest run of the high-weight backend across two full cycles is under 5, and that per-cycle counts still land exactly on 10/2/2 as expected from the weights. Failover for one request computes a *read-only* ranked fallback order after a single SWRR tick, so retrying several backends for one request doesn't perturb the sequence subsequent requests see — same guarantee the old counter-based design had, preserved under the new algorithm.
2. **Live weight updates**: `CanaryLayer` gained `#[derive(Clone)]` backed by `Arc<Mutex<CanaryState>>` (mirroring `BackendPool`'s own "clone freely, all clones share state" pattern) and a new `.update(backends, pools)` method that atomically swaps in a freshly built configuration — the runtime equivalent of constructing a new layer, except every existing clone (including one already wrapped into a running `Application`) picks up the change starting with its very next request. The established idiom is to `.clone()` the layer before `.wrap(...)`-ing it, keeping the clone as a live-control handle (a rollout script, admin endpoint, or scheduled task calls `.update()` on it later).
3. **`BackendPool` integration**: new `WeightedPool { pool: BackendPool, weight: u32 }` plus `CanaryLayer::with_pools(...)` (pure dynamic-group construction) and `.add_pool(pool, weight)` (mix a dynamic group into an otherwise-static layer) — additive API, `WeightedBackend`/`CanaryLayer::new` are unchanged so no existing caller breaks. A pool-backed group's *weight* controls how often that group is picked by the cross-group SWRR tick; which specific member answers is a plain round-robin over `pool.backends()` at selection time, decoupled from the SWRR sequence — so a canary group's pod count can scale up/down freely without ever touching the traffic-split weight. `BackendPool` addresses are bare `"host:port"` with no scheme, so pool-sourced targets are always plain HTTP/1.1 (documented, not silently assumed) — mix in a TLS `WeightedBackend` group alongside a pool if a fixed TLS endpoint is also needed. An empty-at-that-moment pool contributes nothing and falls through to the next group in the fallback order, rather than erroring.

24 rewritten tests (the old ones asserted against the removed flat `rotation` field and no longer applied): SWRR proportionality over a full cycle, the anti-burst regression test described above, live `.update()` swapping both weights and backends↔pools, `Clone` state sharing, `.add_pool()` mixing static and dynamic groups, a live pool's own round-robin cursor alternating across selections, and an empty/zero-weight pool being skipped. Additionally verified end-to-end against two real local HTTP servers through `WithMiddleware`+`TestClient` (not committed — a manual check): 8 requests at a 3:1 weight split landed 6:2, with no consecutive-run burst. Verified across default, `http1`-only, and default+`http1` — 1521 tests green, zero regressions.

### 1.4 Circuit breaker / retry (`src/circuit_breaker/mod.rs`)

Working today: per-backend Closed → Open → HalfOpen state machine; `RetryLayer` retries configurable status codes.

**What is missing:** automatic wiring into `ReverseProxy` (today a caller has to manually check breaker state around each proxied call), a concurrency cap on HalfOpen probes (multiple in-flight requests can all land in HalfOpen and all count as the single trial), and a `rws_circuit_breaker_state{backend}` Prometheus metric so breaker trips are visible without log-grepping.

---

## Part 2 — Cloud provider parity

### 2.1 No GCS object storage backend

`storage-s3` (AWS S3, and S3-compatible R2/MinIO via path-style addressing) and `storage-azure` (Azure Blob, Shared Key + Managed Identity) both exist under `src/storage/`, each with hand-rolled request signing over `http_client::Client` — no vendor SDK. **There is no `storage-gcs` feature.** Of the three major clouds, GCP is the only one without an object storage backend, which is a real gap for anyone running on GKE.

**What to add:** `src/storage/gcs.rs` behind a `storage-gcs` feature — `GcsStorage` implementing the existing `Storage` trait (`put`/`get`/`delete`/`url`), authenticating via either a service-account JSON key (sign with RS256, same primitive `sso`'s JWKS verifier already uses in reverse) or the GKE metadata-server workload identity endpoint (`http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token`) for in-cluster credential-free access — the GCS analogue of `storage-azure`'s Managed Identity path.

### 2.2 No secrets-manager integration

JWT signing keys, DB credentials, and TLS keys are read from plain env vars or files everywhere in the codebase today. On a managed cloud, the idiomatic pattern is Vault, AWS Secrets Manager, or Azure Key Vault — and the current answer is "bridge it yourself with an init container that populates env vars."

**What to add:** a thin `secrets::resolve(value: &str) -> String` abstraction: a config value of the literal form `vault://path#field`, `aws-sm://secret-name`, or `azkv://vault-name/secret-name` is resolved at startup via the corresponding HTTP API (Vault's KV v2 HTTP API, AWS Secrets Manager's `GetSecretValue`, Azure Key Vault's `GET /secrets/{name}`), signed the same way `storage-s3`/`storage-azure` already sign requests; anything not matching one of those prefixes passes through unchanged, so this is additive to every existing `RWS_CONFIG_*`/`RWS_*` env var, not a breaking change.

### ✅ 2.3 No shippable Kubernetes manifests or Helm chart — Done

`docs/deployment/kubernetes.md` and `docs/deployment/kubernetes-ingress.md` contain example YAML in prose, but nothing in the repo itself is a deployable artifact — there is no `k8s/` directory with raw manifests and no `helm/` chart. Anyone adopting rws on K8s today copy-pastes the docs' YAML block and hand-edits it.

**What to add:** a minimal `helm/rws/` chart (`Chart.yaml`, `values.yaml`, `templates/deployment.yaml`, `templates/service.yaml`, `templates/hpa.yaml`) parameterizing image, replica count, resource limits, probe settings, and the `RWS_CONFIG_*` env vars already documented — generated from the same YAML already living in `docs/deployment/kubernetes.md` so the two stay in sync rather than drifting.

**Done as scoped, plus the `PodDisruptionBudget` `docs/deployment/kubernetes.md` also documents** (the gap's own template list named only `deployment.yaml`/`service.yaml`/`hpa.yaml`, but the source-of-truth doc page this chart mirrors has four resources, not three — omitting the PDB template would have left the chart and the doc page it's meant to track out of sync on day one). `helm/rws/` also gained the conventional Helm scaffolding no real chart ships without: `templates/_helpers.tpl` (name/label helpers), `templates/serviceaccount.yaml` (optional, `serviceAccount.create`), `templates/NOTES.txt` (post-install instructions), `.helmignore`, and a chart-level `README.md` documenting every `values.yaml` key.

**Validated as actually-deployable, not just illustrative** — the whole point of this gap entry: `helm lint` passes; `helm template` was rendered under the default values and every toggle combination (`autoscaling.enabled` + `customMetric.enabled`, `serviceAccount.create`, `tls.enabled=false`, `quic.enabled=false`) and each rendering was checked with `kubeconform`, which validates manifests offline against the real Kubernetes OpenAPI schemas (no live cluster needed) — every resource in every combination validated clean. The YAML blocks embedded in `docs/deployment/kubernetes.md` itself were extracted and run through the same `kubeconform` check and are independently valid, confirming the doc page this chart mirrors was already accurate, not merely illustrative.

### 2.4 No HPA custom-metrics example

`/metrics` exposes Prometheus-format counters/histograms (`rws_route_requests_total`, `rws_route_duration_seconds`), which is the prerequisite for Horizontal Pod Autoscaler scaling on request rate or latency rather than just CPU — but nothing in the docs shows the `HorizontalPodAutoscaler` + `prometheus-adapter` wiring to actually do it. Documentation-only gap (no code change), but it's the natural payoff of the metrics work already done and is currently unrealized.

---

## Summary table

| # | Gap | Area | Cloud-specific? | Effort |
|---|---|---|---|---|
| 1.1 | Ingress: TLS to API server, watch API, `pathType: Exact`, class filtering | K8s middleware | No | Medium |
| 1.2 | Service discovery: Consul/etcd/Docker/SRV/weighted-DNS | K8s middleware | No | ✅ Done |
| 1.3 | Canary: live weights, smooth WRR, `BackendPool` integration | K8s middleware | No | ✅ Done |
| 1.4 | Circuit breaker: `ReverseProxy` auto-wiring, HalfOpen cap, metric | K8s middleware | No | Small |
| 2.1 | `storage-gcs` feature | Cloud provider | GCP | Medium |
| 2.2 | `secrets::resolve()` (Vault / AWS SM / Azure KV) | Cloud provider | AWS/Azure/Vault | Medium |
| 2.3 | Helm chart / `k8s/` manifests | Deployment artifact | No | ✅ Done |
| 2.4 | HPA custom-metrics doc example | Docs only | No | Trivial |

---

## Suggested implementation order

1. **`storage-gcs`** — closes the last gap in three-cloud object storage parity; follows the exact `storage-s3`/`storage-azure` pattern already established, so it's low-risk and mechanical.
2. **Helm chart** — ✅ done. Smallest effort, immediately unblocks real adoption (nothing to hand-edit), and forced the `docs/deployment/kubernetes.md` YAML to be validated as actually-deployable rather than illustrative (see §2.3).
3. **Ingress watch API + IngressClass filtering** — ✅ done. The two ingress gaps most likely to bite in a real multi-tenant cluster (stale routes under polling; picking up Ingresses meant for a different controller).
4. **`secrets::resolve()`** — highest effort but closes a recurring enterprise-adoption blocker across all three clouds at once.
5. **Circuit breaker `ReverseProxy` wiring + metric** — small, and makes the already-implemented breaker actually self-service instead of requiring manual integration.
6. **Service discovery and canary remaining items** — ✅ both done (Consul, etcd watch, Docker labels, DNS SRV/weighted DNS; canary's smooth WRR, live weight updates, `BackendPool` integration). All four Part 1 K8s-middleware gaps and the Helm chart in Part 2 are now closed — only `storage-gcs`, `secrets::resolve()`, and the HPA custom-metrics doc example remain open.
