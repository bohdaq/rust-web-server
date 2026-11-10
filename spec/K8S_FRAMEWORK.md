# Kubernetes framework (18.36.0)

rws as a framework for apps on Kubernetes (Spring Cloud Kubernetes,
Quarkus) and for operators (kubebuilder, controller-runtime). Asked for
2026-11-10 after the gap review below.

## Gaps and status

| # | Gap | Status |
|---|---|---|
| 1 | Readiness that checks dependencies; liveness vs startup | ✅ `health` (rws-server): checks per probe, `/livez` `/readyz` `/startupz`, `?verbose`, timeout, off the event loop; Helm `startupProbe` |
| 2 | Shutdown waits for handlers in flight | ✅ `server::drain_requests`, `RWS_CONFIG_SHUTDOWN_TIMEOUT_SECS` (25); `tests/graceful_drain.rs` |
| 3 | ConfigMaps/Secrets from the API, live | ✅ `k8s::config::ConfigWatch`, `k8s://` references |
| 4 | Service discovery through Kubernetes | ✅ `k8s::discovery::service_endpoints` (EndpointSlices → `BackendPool`) |
| 5 | Leader election for apps | ✅ `k8s::leader::LeaderElector`; the Gateway API controller uses it too (rws-k8s feature `leader`) |
| 6 | Deploy tooling | ✅ `rws gen deploy` |
| 7 | Controller runtime | ✅ `k8s::controller` on kube-runtime: `Controller`, `Operator`, finalizers, backoff, metrics, leader election |
| 8 | Admission and conversion webhooks | ✅ `k8s::webhook` + `WebhookCertificate` |
| 9 | CRD and RBAC generation | ✅ `k8s::manifests` (CRDs from `#[derive(CustomResource)]`) |
| 10 | Tests against a real API server | ✅ `k8s::testing::TestCluster`; CI job `kubernetes` (kind) |

## Decisions

- **kube-rs, not hand-rolled.** The gateway controller already depends on
  kube 4.2; its watcher, reflector, finalizer and controller are well
  tested. rws adds what operators write around it.
- **A crate of its own** (`rws-k8s`, facade feature `k8s`): kube and
  schemars stay out of every other build.
- **Leadership lost = exit.** kube's `Controller` can't be stopped and
  restarted from its parts; the replica exits and comes back as a
  follower. Never two leaders reconciling.
- **Manifests as JSON** (valid YAML): no YAML dependency.
- **`rws gen deploy` writes files, not a chart.** The Helm chart is for
  the `rws` server; apps get plain kustomize-ready YAML to own.

## Follow-ups (2026-11-10, branch `k8s-followups`)

- The Gateway API controller's Lease goes through `LeaderElector`; rws-k8s
  is split into `leader` (client + leader election: kube client and
  k8s-openapi only) and `k8s` (everything), so the gateway doesn't pull in
  schemars, rcgen or kube's runtime/derive.
- `starters/operator` and `ci/e2e/operator.sh`: the operator built into an
  image, loaded into kind (k3d locally), installed from its own
  `--manifests`; the API server calls the validating and mutating webhooks
  over TLS with the CA the operator put into the configurations; owned
  ConfigMap, status, finalizer, two replicas with one leading. It found
  three bugs: library apps' `/readyz` never turned ready, `to_yaml` crashed
  `kubectl apply` (now each document starts with `---`), and replicas each
  generating a webhook CA overwrote each other's `caBundle`
  (`WebhookCertificate::shared` keeps one in a Secret).

## Not done

- envtest's binaries (`kube-apiserver` + `etcd`) launched by the tests
  themselves: `TestCluster` takes any kubeconfig; kind in CI.
