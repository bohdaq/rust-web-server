---
title: Kubernetes
description: Deploy rust-web-server on Kubernetes with health probes, metrics scraping, graceful shutdown, and autoscaling.
---

## Health endpoints

`rust-web-server` exposes one endpoint per Kubernetes probe:

| Endpoint | Probe | `200` when |
|---|---|---|
| `GET /livez` (also `GET /healthz`) | Liveness | Every liveness check passes — with none registered, while the process answers |
| `GET /readyz` | Readiness | The server is up and not shutting down, and every readiness check passes; `503` during startup and the SIGTERM drain |
| `GET /startupz` | Startup | Every startup check has passed once (then it stays `200`) |

### Health checks

Register what "alive", "ready" and "started" mean for your app with the `health` module (18.36.0):

```rust
use rust_web_server::health;

health::database("db", pool.clone());                    // readiness: SELECT 1 (model-* features)
health::readiness("payments-api", || ping_payments());   // Err(message) takes the pod out of the Service
health::startup("search-index", || if index_loaded() { Ok(()) } else { Err("loading".into()) });
health::liveness("worker", || if worker_alive() { Ok(()) } else { Err("the worker thread died".into()) });
```

Checks run concurrently, each within `health::set_timeout` (default 2 s; a slower check fails), off the event loop, so a check may block. `?verbose` gives one line per check, as the Kubernetes API server's own probes do:

```text
$ curl -s localhost:7878/readyz?verbose
[+]db ok
[-]payments-api failed: connection refused
readyz check failed
```

Without `?verbose` the body is `OK`, or the failed checks. `health::register_async(probe, name, || async { … })` takes an async check; `health::report(probe)` runs them from code. Keep liveness checks to what a restart fixes: a database outage belongs in readiness, or every pod restarts at once.

## Graceful shutdown

When Kubernetes sends `SIGTERM`, the server:

1. Clears `SERVER_READY` — `/readyz` begins returning `503`, Kubernetes stops sending new traffic.
2. Keeps accepting for `shutdown_delay_secs` (default 5, since 18.27.0): endpoint removal reaches kube-proxy and ingress controllers a moment after the pod is told to stop, and until then they still send it requests. Before 18.27 rws stopped accepting at once and those got a `502`/`503` from the load balancer.
3. Stops accepting new TCP connections.
4. Waits for the requests whose handlers are still running, at most `shutdown_timeout_secs` (default 25, since 18.36.0; `RWS_CONFIG_SHUTDOWN_TIMEOUT_SECS`) — the thread pool's drain on `http1`. Before 18.36 the async servers returned at once and a handler still running was cut off when the process exited. Idle keep-alive connections don't hold it up.
5. Exits cleanly.

Ctrl+C (SIGINT) skips the delay and the wait. Set `terminationGracePeriodSeconds` to at least the delay plus the timeout (`30` with the defaults) to give in-flight requests time to complete before Kubernetes sends `SIGKILL`.

## Generate the manifests

`rws gen deploy --image <image>` writes `deploy/k8s/` for the app in the current project (18.36.0): a Deployment with the startup, readiness and liveness probes above, `POD_NAME`/`POD_NAMESPACE` from the downward API, a 35 s grace period for the shutdown, a non-root read-only container; a Service; a PodDisruptionBudget; a HorizontalPodAutoscaler; a ServiceAccount; and a `kustomization.yaml`:

```sh
rws gen deploy --image ghcr.io/acme/notes:1.4.0 [--port 7878] [--replicas 2] [--max-replicas 10] [--out deploy/k8s]
kubectl apply -k deploy/k8s
```

The name comes from `rws.app.toml` (or `Cargo.toml`). The files are yours to change; like every generator it never overwrites one (`--dry-run` shows them first). For an app that uses the Kubernetes API see [Using the Kubernetes API](/deployment/kubernetes-api/); an operator prints its own RBAC ([Operators](/deployment/kubernetes-operators/#manifests)).

:::tip[Helm chart]
The manifests below are also a Helm chart, published with every release (since 18.2.0):

```bash
kubectl create secret tls rws-tls --cert=tls.crt --key=tls.key
helm install my-rws oci://ghcr.io/bohdaq/charts/rws --version 18.2.0
```

It renders the same `Deployment`, `Service`, `PodDisruptionBudget` and `HorizontalPodAutoscaler`, plus an optional `Ingress` and prometheus-operator `ServiceMonitor`; CI installs it into a kind cluster with TLS on and off. Its values are documented in [`helm/rws/README.md`](https://github.com/bohdaq/rust-web-server/tree/main/helm/rws).

Routes, upstreams and middleware come from `rws.config.toml`; give it to the chart as `configFile` (since 18.27.0). The chart mounts it from a ConfigMap and sets `RWS_CONFIG_FILE`. A new value rolls the pods; with `watch = true` in the file, an edit of the ConfigMap itself is applied in place, within a minute (the kubelet's sync).

```bash
helm install my-rws oci://ghcr.io/bohdaq/charts/rws --set tls.enabled=false --set quic.enabled=false \
  --set-file configFile=rws.config.toml
```
:::

## Deployment YAML

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rws
  labels:
    app: rws
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rws
  template:
    metadata:
      labels:
        app: rws
      annotations:
        # Prometheus scrape annotations — tells Prometheus to scrape this pod.
        prometheus.io/scrape: "true"
        prometheus.io/path: "/metrics"
        prometheus.io/port: "7878"
        prometheus.io/scheme: "https"   # the pod serves TLS
    spec:
      terminationGracePeriodSeconds: 30
      containers:
        - name: rws
          image: ghcr.io/bohdaq/rws:18
          ports:
            - name: https
              containerPort: 7878
              protocol: TCP
            - name: quic
              containerPort: 7878
              protocol: UDP
          env:
            - name: RWS_CONFIG_IP
              value: "0.0.0.0"
            - name: RWS_CONFIG_PORT
              value: "7878"
            - name: RWS_CONFIG_THREAD_COUNT
              value: "4"
            - name: RWS_CONFIG_LOG_FORMAT
              value: "json"
            - name: RWS_CONFIG_TLS_CERT_FILE
              value: "/tls/tls.crt"
            - name: RWS_CONFIG_TLS_KEY_FILE
              value: "/tls/tls.key"
          volumeMounts:
            - name: tls
              mountPath: /tls
              readOnly: true
          livenessProbe:
            httpGet:
              path: /healthz
              port: 7878
              scheme: HTTPS   # the port serves TLS; plain-HTTP probes would fail
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 3
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /readyz
              port: 7878
              scheme: HTTPS   # the port serves TLS; plain-HTTP probes would fail
            initialDelaySeconds: 2
            periodSeconds: 5
            timeoutSeconds: 2
            failureThreshold: 2
          resources:
            requests:
              cpu: "100m"
              memory: "64Mi"
            limits:
              cpu: "500m"
              memory: "256Mi"
      volumes:
        - name: tls
          secret:
            secretName: rws-tls
```

Every `RWS_CONFIG_*` variable can come from a ConfigMap or Secret. Settings that name a file, such as `RWS_CONFIG_TLS_CERT_FILE`, take a path, so mount the Secret as a volume (as above) rather than putting its contents in the variable:

```yaml
envFrom:
  - configMapRef:
      name: rws-config   # e.g. RWS_CONFIG_LOG_FORMAT, RWS_CONFIG_THREAD_COUNT
```

## Service YAML

```yaml
apiVersion: v1
kind: Service
metadata:
  name: rws
spec:
  selector:
    app: rws
  ports:
    - name: https
      protocol: TCP
      port: 443
      targetPort: 7878
    - name: quic
      protocol: UDP
      port: 443
      targetPort: 7878
  type: ClusterIP
```

## PodDisruptionBudget

Keep at least two pods running during rolling updates or node drains:

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: rws-pdb
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: rws
```

## HorizontalPodAutoscaler

Scale on CPU utilisation, or on requests per second per pod. rws exports the
counter `rws_requests_total` at `/metrics`; the
[Prometheus Adapter](https://github.com/kubernetes-sigs/prometheus-adapter)
turns it into the per-pod rate `rws_requests_per_second` in the custom
metrics API. Install it with the rule in
[`helm/prometheus-adapter-values.yaml`](https://github.com/bohdaq/rust-web-server/blob/main/helm/prometheus-adapter-values.yaml):

```bash
helm install prometheus-adapter prometheus-community/prometheus-adapter \
  -n monitoring -f helm/prometheus-adapter-values.yaml   # set prometheus.url first
kubectl get --raw "/apis/custom.metrics.k8s.io/v1beta1/namespaces/default/pods/*/rws_requests_per_second"
```

The rule:

```yaml
rules:
  custom:
    - seriesQuery: 'rws_requests_total{namespace!="",pod!=""}'
      resources:
        overrides:
          namespace: {resource: namespace}
          pod: {resource: pod}
      name:
        matches: "^rws_requests_total$"
        as: "rws_requests_per_second"
      metricsQuery: 'sum(rate(<<.Series>>{<<.LabelMatchers>>}[2m])) by (<<.GroupBy>>)'
```

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rws-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rws
  minReplicas: 2
  maxReplicas: 20
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    # Requests per second per pod, from the Prometheus Adapter rule above.
    - type: Pods
      pods:
        metric:
          name: rws_requests_per_second
        target:
          type: AverageValue
          averageValue: "500"
```

With the Helm chart: `--set autoscaling.enabled=true --set autoscaling.customMetric.enabled=true`.
(Until 18.2.0 this page used an `External` metric named `rws_route_requests_total`, which nothing served.)

## Structured logging for Kubernetes

Access logs are JSON by default (`RWS_CONFIG_LOG_FORMAT=json`, or `log_format = 'json'` in `rws.config.toml`; `combined` gives Combined Log Format, `off`/`none` disables the access log). Kubernetes log aggregators (Fluentd, Fluent Bit, Vector) can then parse and index each field:

```json
{"time":"2026-07-02T12:00:00Z","remote_addr":"10.0.0.1","method":"GET","path":"/api/users","protocol":"HTTP/1.1","status":200,"bytes":1024}
```

:::note[Log to stdout]
`rust-web-server` writes all access logs to stdout (buffered per thread, written at least every 100 ms). Let the container runtime and your log aggregator handle collection and rotation — this is the Kubernetes-native approach.
:::

## Prometheus scraping

With the `prometheus.io/*` annotations on the pod (`prometheus.io/scheme: "https"` when the pod serves TLS), a Prometheus using annotation-based discovery scrapes `GET /metrics` automatically; with the prometheus-operator, enable the chart's `metrics.serviceMonitor` instead. The endpoint returns server-wide counters and — if `MetricsLayer` is enabled — per-route histograms in Prometheus text format. Circuit-breaker state (`rws_circuit_breaker_state{backend}`) is included automatically. The full metric list is on the [Metrics](/features/metrics/) page.

See the [Observability](/deployment/observability) page for example Grafana queries.
