# rws Helm chart

Deploys [`rust-web-server`](https://github.com/bohdaq/rust-web-server) (`rws`) — an HTTP/1.1, HTTP/2, and HTTP/3 web server, reverse proxy, and application framework — on Kubernetes.

This chart renders the manifests documented at [docs/deployment/kubernetes.md](https://rws8.tech/deployment/kubernetes/) (health probes, graceful shutdown, `RWS_CONFIG_*` env vars, Prometheus scraping, autoscaling): a `Deployment`, a `Service`, and optionally a `PodDisruptionBudget`, `HorizontalPodAutoscaler`, `Ingress` and prometheus-operator `ServiceMonitor`. See that page for the semantics behind each field; this README documents the chart's `values.yaml` knobs.

## Installing

Each rws release publishes the chart under the same version (since 18.2.0), using the image `ghcr.io/bohdaq/rws:<version>`:

```sh
helm install my-rws oci://ghcr.io/bohdaq/charts/rws --version 18.2.0
# or from a checkout of this repo:
helm install my-rws ./helm/rws
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
helm lint --strict ./helm/rws
helm template my-rws ./helm/rws | kubeconform -strict -summary   # offline schema validation
```

CI (`.github/workflows/helm.yml`) runs both for several value sets and installs the packaged chart into a kind cluster with TLS on and off, requiring the pods to pass their probes and answer through the Service.

## Values

| Key | Default | Description |
|---|---|---|
| `replicaCount` | `3` | Pod replica count. Ignored (Deployment omits `spec.replicas`) when `autoscaling.enabled` is `true`. |
| `image.repository` | `ghcr.io/bohdaq/rws` | Container image repository. |
| `image.tag` | `""` | Image tag; empty means the chart's `appVersion` (the rws release it was published with). |
| `image.pullPolicy` | `IfNotPresent` | Image pull policy. |
| `imagePullSecrets` | `[]` | List of `{name: ...}` for private registries. |
| `nameOverride` / `fullnameOverride` | `""` | Override the chart-derived resource name. |
| `serviceAccount.create` | `false` | Create a ServiceAccount (e.g. for workload identity / IRSA). |
| `serviceAccount.annotations` | `{}` | Annotations for the created ServiceAccount. |
| `serviceAccount.name` | `""` | ServiceAccount name. When `create` is `false`, this is used as-is (empty = the namespace's `default` ServiceAccount). |
| `podAnnotations` | `{}` | Pod-level annotations. |
| `metrics.podAnnotations` | `true` | Add `prometheus.io/scrape`, `/path`, `/port` and `/scheme` (`https` when `tls.enabled`) to the pod. |
| `metrics.serviceMonitor.enabled` | `false` | Create a prometheus-operator `ServiceMonitor` (needs its CRDs). |
| `metrics.serviceMonitor.interval` / `scrapeTimeout` / `labels` | `30s` / `10s` / `{}` | Scrape settings; `labels` must match your Prometheus's `serviceMonitorSelector`. |
| `metrics.serviceMonitor.tlsConfig` | `insecureSkipVerify: true` | Used when `tls.enabled` — the pod certificate is usually issued for the public name, not the pod IP. |
| `podLabels` | `{}` | Additional pod labels. |
| `terminationGracePeriodSeconds` | `30` | Time to drain in-flight requests after `SIGTERM` before `SIGKILL`. |
| `containerPort` | `7878` | Port the container listens on (HTTP/2+TLS and, if `quic.enabled`, HTTP/3/QUIC share this port). |
| `env` | see `values.yaml` | Map of `RWS_CONFIG_*` env vars rendered as plain `name`/`value` pairs. |
| `extraEnv` | `[]` | Additional raw `{name, value}` / `{name, valueFrom}` entries, for anything `env` can't express (e.g. a Secret reference). |
| `envFrom` | `[]` | Whole Secrets/ConfigMaps to import as env vars. |
| `tls.enabled` | `true` | Mount a TLS Secret and set `RWS_CONFIG_TLS_CERT_FILE`/`RWS_CONFIG_TLS_KEY_FILE`; probes, scrape annotations and the Service port name follow (`https`/`http`). |
| `tls.secretName` | `rws-tls` | Name of an existing `kubernetes.io/tls` Secret in the release namespace — **not created by this chart**. |
| `tls.certPath` / `tls.keyPath` / `tls.mountPath` | `/tls/tls.crt` / `/tls/tls.key` / `/tls` | Where the Secret is mounted and the paths passed to `rws`. |
| `quic.enabled` | `true` | Add the UDP `quic` container/service port for HTTP/3. Set `false` if your image is built with `--features http2` or `--features http1`. |
| `service.type` | `ClusterIP` | Service type. |
| `service.httpsPort` / `service.httpPort` / `service.quicPort` | `443` / `80` / `443` | Service ports (all target `containerPort`): `httpsPort` with `tls.enabled`, `httpPort` without. |
| `ingress.enabled` | `false` | Create an `Ingress` to the Service. With `tls.enabled` the backend speaks HTTPS: the chart adds ingress-nginx's `backend-protocol: HTTPS` annotation; other controllers need their own equivalent. |
| `ingress.className` / `annotations` / `hosts` / `tls` | `""` / `{}` / `rws.example.com` `/` / `[]` | Standard Ingress fields. |
| `service.annotations` | `{}` | Service annotations (e.g. for a cloud load-balancer controller). |
| `resources` | `100m`/`64Mi` requests, `500m`/`256Mi` limits | Container resource requests/limits. |
| `livenessProbe.*` | path `/healthz`, see `values.yaml` | Liveness probe settings. |
| `readinessProbe.*` | path `/readyz`, see `values.yaml` | Readiness probe settings. |
| `podDisruptionBudget.enabled` | `true` | Create a PodDisruptionBudget. |
| `podDisruptionBudget.minAvailable` | `2` | Minimum available pods during voluntary disruptions. |
| `autoscaling.enabled` | `false` | Create a HorizontalPodAutoscaler (and omit `Deployment.spec.replicas`). |
| `autoscaling.minReplicas` / `maxReplicas` | `2` / `20` | HPA replica bounds. |
| `autoscaling.targetCPUUtilizationPercentage` | `70` | CPU-based scaling target. Set to `0`/`null` to omit the CPU metric. |
| `autoscaling.customMetric.enabled` | `false` | Also scale on requests per second per pod (a `Pods` metric). Needs the Prometheus Adapter with the rule in [`../prometheus-adapter-values.yaml`](../prometheus-adapter-values.yaml). |
| `autoscaling.customMetric.name` | `rws_requests_per_second` | Metric name, as the adapter rule exposes it. |
| `autoscaling.customMetric.averageValue` | `"500"` | Target requests per second per pod. |
| `nodeSelector` / `tolerations` / `affinity` | `{}` / `[]` / `{}` | Standard pod scheduling controls. |
| `podSecurityContext` / `securityContext` | `{}` | Pod-level and container-level `securityContext`. |

## What this chart deliberately does not do

- **Does not create the TLS Secret.** Certificate provisioning (cert-manager, manual, etc.) is out of scope — see [docs/deployment/https-tls.md](https://rws8.tech/features/https-tls/) and [docs/deployment/acme.md](https://rws8.tech/features/acme/).
- **Does not install the Prometheus Adapter** needed for `autoscaling.customMetric` — it's cluster-level infrastructure; [`../prometheus-adapter-values.yaml`](../prometheus-adapter-values.yaml) has the values for the `prometheus-community/prometheus-adapter` chart.
