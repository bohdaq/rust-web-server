[Read Me](../README.md) > [Spec](.) > Gaps V4

# Gaps V4 — Kubernetes Adoption & Cloud Providers

`K8S_ROADMAP.md` tracked the minimum for a pod to run and be probed correctly — all six items there are closed (0.0.0.0 bind, `/healthz`/`/readyz`, SIGTERM drain, Dockerfile, JSON logs, `/metrics`). `GAPS.md`'s "Kubernetes / cloud-native gaps" section then added ingress, service discovery, canary routing, and circuit breaking as middleware — all four shipped, each with its own "Remaining" list. This document (audited against v17.94.0) rolls those remaining items up alongside the gaps that are specific to running on a *managed cloud provider* (AWS/GCP/Azure) rather than bare Kubernetes: object storage parity, secrets management, and shippable deployment artifacts.

---

## Part 1 — Kubernetes middleware, remaining items

### 1.1 Ingress controller (`src/ingress/mod.rs`)

Working today: polls `/apis/networking.k8s.io/v1/ingresses`, builds a live route table, routes to `{service}.{namespace}.svc.cluster.local:{port}`.

**What is missing:**
- TLS to `kubernetes.default.svc` — the watcher talks to the API server over plain HTTP today; needs a rustls client config trusting the in-cluster CA (`/var/run/secrets/kubernetes.io/serviceaccount/ca.crt`).
- Watch API (`?watch=true`) instead of fixed-interval polling — polling means route changes lag by up to the poll interval and the watcher does needless work on quiet clusters.
- `pathType: Exact` support — only prefix matching is implemented.
- IngressClass filtering — the watcher currently picks up every Ingress object cluster-wide regardless of `spec.ingressClassName`, which breaks multi-controller clusters (e.g. rws running alongside nginx-ingress).

### 1.2 Service discovery (`src/service_discovery/mod.rs`)

Working today: `Static`, `EnvPrefix`, `File`, `Dns` sources feeding a shared `Arc<RwLock<Vec<String>>>`.

**What is missing:** Consul HTTP API source, etcd watch source, Docker label discovery, DNS SRV record support (needed for headless Services that publish port info via SRV, not just A records), weighted DNS.

### 1.3 Traffic splitting / canary (`src/canary/mod.rs`)

Working today: `CanaryLayer` distributes requests across backends by static weight via a precomputed rotation vec.

**What is missing:** live weight updates without a restart (today weights are baked in at construction), smooth weighted round-robin (current scheme can burst same-backend requests within a rotation cycle for skewed weights), integration with `BackendPool` so canary targets can come from a dynamic discovery source instead of a fixed list.

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
| 1.2 | Service discovery: Consul/etcd/Docker/SRV/weighted-DNS | K8s middleware | No | Medium |
| 1.3 | Canary: live weights, smooth WRR, `BackendPool` integration | K8s middleware | No | Small–Medium |
| 1.4 | Circuit breaker: `ReverseProxy` auto-wiring, HalfOpen cap, metric | K8s middleware | No | Small |
| 2.1 | `storage-gcs` feature | Cloud provider | GCP | Medium |
| 2.2 | `secrets::resolve()` (Vault / AWS SM / Azure KV) | Cloud provider | AWS/Azure/Vault | Medium |
| 2.3 | Helm chart / `k8s/` manifests | Deployment artifact | No | ✅ Done |
| 2.4 | HPA custom-metrics doc example | Docs only | No | Trivial |

---

## Suggested implementation order

1. **`storage-gcs`** — closes the last gap in three-cloud object storage parity; follows the exact `storage-s3`/`storage-azure` pattern already established, so it's low-risk and mechanical.
2. **Helm chart** — ✅ done. Smallest effort, immediately unblocks real adoption (nothing to hand-edit), and forced the `docs/deployment/kubernetes.md` YAML to be validated as actually-deployable rather than illustrative (see §2.3).
3. **Ingress watch API + IngressClass filtering** — the two ingress gaps most likely to bite in a real multi-tenant cluster (stale routes under polling; picking up Ingresses meant for a different controller).
4. **`secrets::resolve()`** — highest effort but closes a recurring enterprise-adoption blocker across all three clouds at once.
5. **Circuit breaker `ReverseProxy` wiring + metric** — small, and makes the already-implemented breaker actually self-service instead of requiring manual integration.
6. **Service discovery / canary remaining items** — lowest urgency; current sources (Static/EnvPrefix/File/DNS) and static-weight canary already cover the common K8s case (Kubernetes Service DNS + a fixed weight split).
