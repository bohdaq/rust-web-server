# rws Helm chart

Deploys [`rust-web-server`](https://github.com/bohdaq/rust-web-server) (`rws`) — an HTTP/1.1, HTTP/2, and HTTP/3 web server, reverse proxy, and application framework — on Kubernetes.

This chart renders exactly the manifests documented at [docs/deployment/kubernetes.md](https://rws8.tech/deployment/kubernetes/) (health probes, graceful shutdown, `RWS_CONFIG_*` env vars, Prometheus scraping, autoscaling): a `Deployment`, a `Service`, an optional `PodDisruptionBudget`, and an optional `HorizontalPodAutoscaler`. See that page for the semantics behind each field; this README documents the chart's `values.yaml` knobs.

## Installing

```sh
# From a local checkout of this repo:
helm install my-rws ./helm/rws \
  --set image.repository=ghcr.io/your-org/rws \
  --set image.tag=17.98.0
```

TLS is on by default and expects a Secret containing `tls.crt`/`tls.key` to already exist in the release namespace:

```sh
kubectl create secret tls rws-tls --cert=path/to/tls.crt --key=path/to/tls.key
```

To run plain HTTP/1.1 instead (e.g. behind a TLS-terminating load balancer or ingress controller), disable TLS and QUIC:

```sh
helm install my-rws ./helm/rws --set tls.enabled=false --set quic.enabled=false
```

## Validating before you install

```sh
helm lint ./helm/rws
helm template my-rws ./helm/rws | kubeconform -summary   # offline schema validation, no cluster needed
helm template my-rws ./helm/rws --dry-run                # or: helm install --dry-run
```

## Values

| Key | Default | Description |
|---|---|---|
| `replicaCount` | `3` | Pod replica count. Ignored (Deployment omits `spec.replicas`) when `autoscaling.enabled` is `true`. |
| `image.repository` | `ghcr.io/your-org/rws` | Container image repository — set this to your actual registry/image. |
| `image.tag` | `latest` | Image tag. Falls back to `.Chart.AppVersion` if left empty. |
| `image.pullPolicy` | `IfNotPresent` | Image pull policy. |
| `imagePullSecrets` | `[]` | List of `{name: ...}` for private registries. |
| `nameOverride` / `fullnameOverride` | `""` | Override the chart-derived resource name. |
| `serviceAccount.create` | `false` | Create a ServiceAccount (e.g. for workload identity / IRSA). |
| `serviceAccount.annotations` | `{}` | Annotations for the created ServiceAccount. |
| `serviceAccount.name` | `""` | ServiceAccount name. When `create` is `false`, this is used as-is (empty = the namespace's `default` ServiceAccount). |
| `podAnnotations` | Prometheus scrape annotations | Pod-level annotations. |
| `podLabels` | `{}` | Additional pod labels. |
| `terminationGracePeriodSeconds` | `30` | Time to drain in-flight requests after `SIGTERM` before `SIGKILL`. |
| `containerPort` | `7878` | Port the container listens on (HTTP/2+TLS and, if `quic.enabled`, HTTP/3/QUIC share this port). |
| `env` | see `values.yaml` | Map of `RWS_CONFIG_*` env vars rendered as plain `name`/`value` pairs. |
| `extraEnv` | `[]` | Additional raw `{name, value}` / `{name, valueFrom}` entries, for anything `env` can't express (e.g. a Secret reference). |
| `envFrom` | `[]` | Whole Secrets/ConfigMaps to import as env vars. |
| `tls.enabled` | `true` | Mount a TLS Secret and set `RWS_CONFIG_TLS_CERT_FILE`/`RWS_CONFIG_TLS_KEY_FILE`. |
| `tls.secretName` | `rws-tls` | Name of an existing `kubernetes.io/tls` Secret in the release namespace — **not created by this chart**. |
| `tls.certPath` / `tls.keyPath` / `tls.mountPath` | `/tls/tls.crt` / `/tls/tls.key` / `/tls` | Where the Secret is mounted and the paths passed to `rws`. |
| `quic.enabled` | `true` | Add the UDP `quic` container/service port for HTTP/3. Set `false` if your image is built with `--features http2` or `--features http1`. |
| `service.type` | `ClusterIP` | Service type. |
| `service.httpsPort` / `service.quicPort` | `443` / `443` | Service ports (both target `containerPort`). |
| `service.annotations` | `{}` | Service annotations (e.g. for a cloud load-balancer controller). |
| `resources` | `100m`/`64Mi` requests, `500m`/`256Mi` limits | Container resource requests/limits. |
| `livenessProbe.*` | path `/healthz`, see `values.yaml` | Liveness probe settings. |
| `readinessProbe.*` | path `/readyz`, see `values.yaml` | Readiness probe settings. |
| `podDisruptionBudget.enabled` | `true` | Create a PodDisruptionBudget. |
| `podDisruptionBudget.minAvailable` | `2` | Minimum available pods during voluntary disruptions. |
| `autoscaling.enabled` | `false` | Create a HorizontalPodAutoscaler (and omit `Deployment.spec.replicas`). |
| `autoscaling.minReplicas` / `maxReplicas` | `2` / `20` | HPA replica bounds. |
| `autoscaling.targetCPUUtilizationPercentage` | `70` | CPU-based scaling target. Set to `0`/`null` to omit the CPU metric. |
| `autoscaling.customMetric.enabled` | `false` | Add a custom-metric target using a value `MetricsLayer` exposes at `/metrics` (requires the Prometheus Adapter or KEDA). |
| `autoscaling.customMetric.name` | `rws_route_requests_total` | Metric name. |
| `autoscaling.customMetric.averageValue` | `"1000"` | Target average value per pod. |
| `nodeSelector` / `tolerations` / `affinity` | `{}` / `[]` / `{}` | Standard pod scheduling controls. |
| `podSecurityContext` / `securityContext` | `{}` | Pod-level and container-level `securityContext`. |

## What this chart deliberately does not do

- **Does not create the TLS Secret.** Certificate provisioning (cert-manager, manual, etc.) is out of scope — see [docs/deployment/https-tls.md](https://rws8.tech/features/https-tls/) and [docs/deployment/acme.md](https://rws8.tech/features/acme/).
- **Does not deploy an Ingress resource.** `rws` terminates TLS and speaks HTTP/3 itself; fronting it with a Kubernetes `Ingress` is a separate, optional choice covered in [docs/deployment/kubernetes-ingress.md](https://rws8.tech/deployment/kubernetes-ingress/) (a different component — `KubernetesIngressWatcher`/`IngressRouter` — for routing *to* other services, not exposing this one).
- **Does not wire up the Prometheus Adapter/KEDA** needed for `autoscaling.customMetric` — that's cluster-level infrastructure this chart can't install on your behalf.
